//! Whether a source's series is the work: its names compared once the ways sources spell one
//! name are folded away, the words that mark another edition, and the facts a near match needs
//! before it is taken.
//!
//! A series is the work only when its name is one of the work's and nothing about it disagrees.
//! A name that differs in spelling alone also needs the year or a maker to agree; a name with
//! the subtitle missing needs both. A name with an edition word ("Full Color", "Side Story") is
//! another edition: never taken, only offered for the person to confirm. A one-shot, a pilot
//! or a single chapter is never a serialised work.

use std::collections::HashSet;

use kaeri_core::{
    books::{HitAmount, SeriesHit, SourceChapter},
    media::{Medium, ReadingFormat},
};

/// What is known of a work, to judge a source's series by.
#[derive(Debug, Clone)]
pub(crate) struct Work {
    /// The names it goes by, the one shown first.
    pub names: Vec<String>,
    pub romaji: Option<String>,
    pub english: Option<String>,
    pub native: Option<String>,
    pub medium: Medium,
    /// The catalogue lists it as a one-shot.
    pub one_shot: bool,
    /// The year it started.
    pub year: Option<u32>,
    /// Its chapters, once it is finished or the catalogue counts them.
    pub chapters: Option<u32>,
    /// Who wrote and drew it.
    pub authors: Vec<String>,
}

impl Work {
    /// A work known by its names alone.
    pub(crate) fn named(names: Vec<String>, medium: Medium) -> Self {
        Self {
            names,
            romaji: None,
            english: None,
            native: None,
            medium,
            one_shot: false,
            year: None,
            chapters: None,
            authors: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Names

/// A name folded for comparison: case, punctuation, apostrophes, accents and macrons, long
/// vowels (ou, oo and ō are o), the particles "ha" and "wo" as "wa" and "o", articles, and the
/// spaces and hyphens between words, so "Tensai-tachi no Ren'ai" and "Tensaitachi no Renai"
/// are one name.
pub(crate) fn key(name: &str) -> String {
    let folded: String = name.chars().filter_map(fold).collect();
    let normal = kaeri_meta::normalise(&folded);
    let words: Vec<&str> = normal
        .split_whitespace()
        .map(|w| match w {
            "ha" => "wa",
            "wo" => "o",
            w => w,
        })
        .collect();
    let joined = words.concat();
    joined.replace("ou", "o").replace("oo", "o").replace("uu", "u")
}

/// One character with its accent or macron taken off; apostrophes go.
fn fold(c: char) -> Option<char> {
    let lower = c.to_lowercase().next().unwrap_or(c);
    Some(match lower {
        '\'' | '\u{2019}' | '\u{2018}' | '\u{02bc}' | '`' => return None,
        'ā' | 'â' | 'á' | 'à' | 'ä' | 'ã' => 'a',
        'ē' | 'ê' | 'é' | 'è' | 'ë' => 'e',
        'ī' | 'î' | 'í' | 'ì' | 'ï' => 'i',
        'ō' | 'ô' | 'ó' | 'ò' | 'ö' | 'õ' => 'o',
        'ū' | 'û' | 'ú' | 'ù' | 'ü' => 'u',
        other => other,
    })
}

/// A name before its subtitle: what comes before ":", " - ", "~" or a bracket, when that is a
/// name of its own.
pub(crate) fn head(name: &str) -> Option<&str> {
    const MARKS: [&str; 8] = [":", " - ", " – ", " — ", "~", "～", "(", "（"];
    let cut = MARKS.iter().filter_map(|mark| name.find(mark)).min()?;
    let head = name[..cut].trim();
    (key(head).chars().count() >= 3).then_some(head)
}

/// A name as words alone, for a source whose search finds nothing once there is punctuation in
/// it: "Lantern-Ferry: Side Trips" is "Lantern Ferry Side Trips".
pub(crate) fn plain(name: &str) -> String {
    let words: String = name
        .chars()
        .filter_map(|c| match c {
            '\'' | '\u{2019}' | '\u{2018}' | '\u{02bc}' | '`' => None,
            c if c.is_alphanumeric() => Some(c),
            _ => Some(' '),
        })
        .collect();
    words.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The word most likely to find the work by itself: the longest word its romaji and English
/// names share, else the longest word of its romaji.
pub(crate) fn key_word(work: &Work) -> Option<String> {
    let words = |name: &Option<String>| -> Vec<String> {
        name.as_deref()
            .map(|n| plain(n).split(' ').map(str::to_owned).collect())
            .unwrap_or_default()
    };
    let (romaji, english) = (words(&work.romaji), words(&work.english));
    let latin = |w: &&String| w.chars().all(|c| c.is_ascii_alphanumeric());
    let shared: Vec<&String> = romaji
        .iter()
        .filter(latin)
        .filter(|w| english.iter().any(|e| key(e) == key(w)))
        .collect();
    let longest = |words: Vec<&String>, least: usize| {
        words
            .into_iter()
            .filter(|w| w.chars().count() >= least && !COMMON.contains(&key(w).as_str()))
            .max_by_key(|w| w.chars().count())
            .cloned()
    };
    longest(shared, 4).or_else(|| longest(romaji.iter().filter(latin).collect(), 5))
}

/// Words too common to find a work by.
const COMMON: [&str; 12] = [
    "sama", "chan", "kun", "san", "with", "from", "world", "story", "love", "isekai", "this",
    "that",
];

/// How a source's series' name compares with the work's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Title {
    /// One of the work's names, once normalised.
    Same,
    /// One of the work's names spelt another way.
    Spelt,
    /// One of the work's names with its subtitle dropped or added.
    Subtitle,
    /// One of the work's names with an edition word: another edition of it.
    Edition(Edition),
    Other,
}

/// What an edition word says a series is, beside the work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Edition {
    Colour,
    Official,
    Digital,
    Novel,
    Manga,
    Anthology,
    Doujinshi,
    SpinOff,
    OneShot,
    SideStory,
}

/// The words that mark an edition, as normalised words.
const EDITIONS: [(&[&str], Edition); 32] = [
    (&["fan", "colored"], Edition::Colour),
    (&["fan", "coloured"], Edition::Colour),
    (&["fan", "color"], Edition::Colour),
    (&["fan", "colour"], Edition::Colour),
    (&["full", "color"], Edition::Colour),
    (&["full", "colour"], Edition::Colour),
    (&["colored"], Edition::Colour),
    (&["coloured"], Edition::Colour),
    (&["color"], Edition::Colour),
    (&["colour"], Edition::Colour),
    (&["official"], Edition::Official),
    (&["digital"], Edition::Digital),
    (&["light", "novel"], Edition::Novel),
    (&["web", "novel"], Edition::Novel),
    (&["novel"], Edition::Novel),
    (&["manga"], Edition::Manga),
    (&["comic"], Edition::Manga),
    (&["anthology"], Edition::Anthology),
    (&["doujinshi"], Edition::Doujinshi),
    (&["doujin"], Edition::Doujinshi),
    (&["spin", "off"], Edition::SpinOff),
    (&["spinoff"], Edition::SpinOff),
    (&["one", "shot"], Edition::OneShot),
    (&["oneshot"], Edition::OneShot),
    (&["pilot"], Edition::OneShot),
    (&["side", "story"], Edition::SideStory),
    (&["side", "stories"], Edition::SideStory),
    (&["gaiden"], Edition::SideStory),
    (&["extra", "story"], Edition::SideStory),
    (&["pre", "serialization"], Edition::OneShot),
    (&["preserialization"], Edition::OneShot),
    (&["prototype"], Edition::OneShot),
];

/// Whether an edition word marks another edition of a work in `medium`: "Novel" beside a
/// drawn work and "Manga" beside a novel, never the medium's own name.
fn marks(edition: Edition, medium: Medium) -> bool {
    match edition {
        Edition::Novel => medium != Medium::Novel,
        Edition::Manga => medium == Medium::Novel,
        _ => true,
    }
}

/// Take the edition words out of `words` that are not in `ours`, saying the first taken.
fn without_editions(
    words: &[&str],
    ours: &HashSet<&str>,
    medium: Medium,
) -> (Vec<String>, Option<Edition>) {
    let mut out: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
    let mut found = None;
    for (phrase, edition) in EDITIONS {
        if !marks(edition, medium) || phrase.iter().all(|w| ours.contains(w)) {
            continue;
        }
        let n = phrase.len();
        let mut i = 0;
        while i + n <= out.len() {
            if out[i..i + n].iter().map(String::as_str).eq(phrase.iter().copied()) {
                out.drain(i..i + n);
                found.get_or_insert(edition);
            } else {
                i += 1;
            }
        }
    }
    (out, found)
}

/// How `theirs`, a series' name, compares with the work's names.
pub(crate) fn title(work: &Work, theirs: &str) -> Title {
    let mut best = Title::Other;
    let rank = |t: Title| match t {
        Title::Same => 4,
        Title::Spelt => 3,
        Title::Subtitle => 2,
        Title::Edition(_) => 1,
        Title::Other => 0,
    };
    for ours in &work.names {
        let found = compare(ours, theirs, work.medium);
        if rank(found) > rank(best) {
            best = found;
        }
    }
    best
}

/// The best of a hit's names against the work's.
pub(crate) fn hit_title(work: &Work, hit: &SeriesHit) -> Title {
    // A series named as another edition is that edition, whatever its other names say: a
    // colored edition often lists the plain name among them.
    let own = title(work, &hit.title);
    if matches!(own, Title::Edition(_)) {
        return own;
    }
    std::iter::once(&hit.title)
        .chain(&hit.synonyms)
        .map(|name| title(work, name))
        .max_by_key(|t| match t {
            Title::Same => 4,
            Title::Spelt => 3,
            Title::Subtitle => 2,
            Title::Edition(_) => 1,
            Title::Other => 0,
        })
        .unwrap_or(Title::Other)
}

fn compare(ours: &str, theirs: &str, medium: Medium) -> Title {
    let (normal_ours, normal_theirs) =
        (kaeri_meta::normalise(ours), kaeri_meta::normalise(theirs));
    if normal_ours.is_empty() || normal_theirs.is_empty() {
        return Title::Other;
    }
    if normal_ours == normal_theirs {
        return Title::Same;
    }
    // An edition word first: "Name - Full Color" is never the name with a subtitle.
    let our_words: HashSet<&str> = normal_ours.split_whitespace().collect();
    let their_words: Vec<&str> = normal_theirs.split_whitespace().collect();
    let (rest, edition) = without_editions(&their_words, &our_words, medium);
    if let Some(edition) = edition {
        let rest = rest.join(" ");
        let ours_key = key(ours);
        let heads = head(ours).map(key);
        if !rest.is_empty() && (key(&rest) == ours_key || heads.as_deref() == Some(&key(&rest)))
        {
            return Title::Edition(edition);
        }
        return Title::Other;
    }
    let (key_ours, key_theirs) = (key(ours), key(theirs));
    if key_ours == key_theirs {
        return Title::Spelt;
    }
    let head_ours = head(ours).map(key);
    let head_theirs = head(theirs).map(key);
    if head_ours.as_deref() == Some(key_theirs.as_str())
        || head_theirs.as_deref() == Some(key_ours.as_str())
    {
        return Title::Subtitle;
    }
    Title::Other
}

// ---------------------------------------------------------------------------------------------
// Facts

/// What a source says about one of its series, from its search and, when asked, its page and
/// chapter list.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Facts {
    pub format: Option<ReadingFormat>,
    pub year: Option<u32>,
    pub authors: Vec<String>,
    /// Its chapters, each counted once whatever the versions.
    pub chapters: Option<u32>,
    /// Its one chapter is named a one-shot or a pilot.
    pub named_one_shot: bool,
    /// The first chapter it has, when its chapters were counted and are numbered.
    pub first: Option<f64>,
    /// The last chapter it has, likewise: extras and parts add to a count, never to this.
    pub last: Option<f64>,
}

impl Facts {
    pub(crate) fn of_hit(hit: &SeriesHit) -> Self {
        let chapters = match hit.amount {
            Some(HitAmount::Chapters(n) | HitAmount::Episodes(n)) => Some(n),
            _ => None,
        };
        Self { format: hit.format.clone(), year: hit.year, chapters, ..Self::default() }
    }

    /// Count a chapter list: each chapter once, and whether its only one is a one-shot.
    pub(crate) fn count(&mut self, chapters: &[SourceChapter]) {
        let labels = chapters.iter().map(|c| {
            let number = c.number.map(|n| n.to_string());
            (number, c.number_raw.as_str(), c.title.as_deref())
        });
        self.count_labels(labels);
    }

    /// Count chapters given as their number, label and title.
    pub(crate) fn count_labels<'a>(
        &mut self,
        chapters: impl Iterator<Item = (Option<String>, &'a str, Option<&'a str>)>,
    ) {
        let mut seen = HashSet::new();
        let mut one_shot = false;
        let (mut first, mut last): (Option<f64>, Option<f64>) = (None, None);
        for (number, label, title) in chapters {
            if let Ok(n) = number.as_deref().unwrap_or(label).trim().parse::<f64>() {
                first = Some(first.map_or(n, |f| f.min(n)));
                last = Some(last.map_or(n, |l| l.max(n)));
            }
            let id = number.unwrap_or_else(|| label.trim().to_lowercase());
            seen.insert(id);
            let words = format!("{label} {}", title.unwrap_or_default()).to_lowercase();
            one_shot |=
                ["oneshot", "one-shot", "one shot", "pilot"].iter().any(|w| words.contains(w));
        }
        self.chapters = Some(u32::try_from(seen.len()).unwrap_or(u32::MAX));
        self.named_one_shot = seen.len() == 1 && one_shot;
        self.first = first;
        self.last = last;
    }

    /// Its chapters begin at the work's start: chapter 0 or 1, or nothing numbered to say.
    pub(crate) fn has_the_start(&self) -> bool {
        self.first.is_none_or(|n| n <= 1.0)
    }
}

/// What a series' facts say about it being the work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fit {
    /// Nothing disagrees, and both the year and a maker agree.
    Sure,
    /// Nothing disagrees, and the year or a maker agrees.
    Backed,
    /// Nothing disagrees, and nothing says it is the work either.
    Unbacked,
    /// A one-shot, a pilot or a single chapter of a serialised work.
    OneShot,
    /// Its medium, year, makers or chapter count say it is another work.
    Wrong,
}

/// How many chapters a source can list beyond the work's count and still be the work: extras
/// and chapters split in parts.
fn slack(total: u32) -> u32 {
    (total / 5).max(10)
}

pub(crate) fn fit(work: &Work, facts: &Facts) -> Fit {
    let drawn = |f: &ReadingFormat| {
        matches!(f, ReadingFormat::Manga | ReadingFormat::Webtoon | ReadingFormat::OneShot)
    };
    match &facts.format {
        Some(ReadingFormat::Novel) if work.medium != Medium::Novel => return Fit::Wrong,
        Some(f) if drawn(f) && work.medium == Medium::Novel => return Fit::Wrong,
        _ => {}
    }
    if !work.one_shot && work.chapters != Some(1) {
        let single = facts.chapters == Some(1) && work.chapters.is_some_and(|t| t >= 2);
        if facts.format == Some(ReadingFormat::OneShot) || facts.named_one_shot || single {
            return Fit::OneShot;
        }
    }
    let year = match (work.year, facts.year) {
        (Some(ours), Some(theirs)) if ours.abs_diff(theirs) > 1 => return Fit::Wrong,
        (Some(_), Some(_)) => true,
        _ => false,
    };
    let makers = match (work.authors.is_empty(), facts.authors.is_empty()) {
        (false, false) if !same_maker(&work.authors, &facts.authors) => return Fit::Wrong,
        (false, false) => true,
        _ => false,
    };
    // The last chapter's number when the list has one: a source that adds extras, parts and
    // side chapters to a count still numbers the work's chapters as the work does.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let theirs = facts.last.map(|n| n.max(0.0).floor() as u32).or(facts.chapters);
    if let (Some(total), Some(theirs)) = (work.chapters, theirs)
        && theirs > total + slack(total)
    {
        return Fit::Wrong;
    }
    match (year, makers) {
        (true, true) => Fit::Sure,
        (true, false) | (false, true) => Fit::Backed,
        (false, false) => Fit::Unbacked,
    }
}

/// Whether two lists of makers share a person, whichever order their names are written in:
/// "Aka Lantern" and "LANTERN Aka" are one.
fn same_maker(ours: &[String], theirs: &[String]) -> bool {
    let words = |name: &String| -> Vec<String> {
        let mut words: Vec<String> =
            kaeri_meta::normalise(&name.chars().filter_map(fold).collect::<String>())
                .split_whitespace()
                .map(str::to_owned)
                .collect();
        words.sort();
        words
    };
    let ours: Vec<Vec<String>> = ours.iter().map(words).filter(|w| !w.is_empty()).collect();
    theirs.iter().map(words).any(|t| !t.is_empty() && ours.contains(&t))
}

/// Whether a series whose name compares as `title` and whose facts fit as `fit` is the work.
pub(crate) fn taken(title: Title, fit: Fit) -> bool {
    match title {
        Title::Same => matches!(fit, Fit::Sure | Fit::Backed | Fit::Unbacked),
        Title::Spelt => matches!(fit, Fit::Sure | Fit::Backed),
        Title::Subtitle => fit == Fit::Sure,
        Title::Edition(_) | Title::Other => false,
    }
}

/// How far a chapter count is from the work's, for picking between series that both fit: the
/// nearest first, and with no count known, the one with most.
pub(crate) fn distance(work: &Work, chapters: Option<u32>) -> u32 {
    match (work.chapters, chapters) {
        (Some(total), Some(n)) => total.abs_diff(n),
        (None, Some(n)) => u32::MAX / 2 - n.min(u32::MAX / 2),
        (_, None) => u32::MAX,
    }
}

/// The makers in a catalogue's staff: who wrote or drew it, not who translated or lettered it.
pub(crate) fn is_maker(role: &str) -> bool {
    let role = role.to_lowercase();
    ["story", "art", "creator", "author"].iter().any(|w| role.contains(w))
        && !role.contains('(')
        && !["assist", "letter", "translat", "design", "touch"].iter().any(|w| role.contains(w))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A drawn work from 2015 with 120 chapters, by Mira Tamsin.
    fn council() -> Work {
        Work {
            names: vec![
                "Starlit Council: Beyond the Night Hours".into(),
                "Hoshizora-tachi no Ren'ai Kaigi".into(),
            ],
            romaji: Some("Hoshizora-tachi no Ren'ai Kaigi".into()),
            english: Some("Starlit Council: Beyond the Night Hours".into()),
            native: None,
            medium: Medium::Manga,
            one_shot: false,
            year: Some(2015),
            chapters: Some(120),
            authors: vec!["Mira Tamsin".into()],
        }
    }

    fn named(name: &str) -> Work {
        Work::named(vec![name.into()], Medium::Manga)
    }

    #[test]
    fn spellings_of_one_name_fold_to_one_key() {
        let same = |a: &str, b: &str| assert_eq!(key(a), key(b), "{a} / {b}");
        // Punctuation, and hyphens joining a compound.
        same("Hoshizora-tachi no Kaigi", "Hoshizoratachi no Kaigi");
        same("Lantern Ferry: Night Run", "Lantern Ferry - Night Run");
        same("Lantern Ferry!?", "lantern ferry");
        // Apostrophes, straight and curled.
        same("Ren'ai Kaigi", "Renai Kaigi");
        same("Wayfarer’s Road", "Wayfarers Road");
        // Macrons and long vowels.
        same("Kōri no Shiro", "Kouri no Shiro");
        same("Kōri no Shiro", "Kori no Shiro");
        same("Yuuki no Uta", "Yūki no Uta");
        // The particle は written as it sounds or as it is spelt.
        same("Kaigi wa Odoru", "Kaigi ha Odoru");
        // Leading articles.
        same("The Lantern Ferry", "Lantern Ferry");
        // Spaces where another source has none.
        same("Ren ai Kaigi", "Renai Kaigi");
        assert_ne!(key("Lantern Ferry"), key("Lantern Ferry 2"));
        assert_ne!(key("Lantern Ferry"), key("Lantern Ferry Season 2"));
    }

    #[test]
    fn a_name_has_a_head_before_its_subtitle() {
        assert_eq!(head("Starlit Council: Beyond the Night Hours"), Some("Starlit Council"));
        assert_eq!(head("Lantern Ferry - Night Run"), Some("Lantern Ferry"));
        assert_eq!(head("Lantern Ferry ~Night Run~"), Some("Lantern Ferry"));
        assert_eq!(head("Lantern-Ferry"), None, "a hyphen in a word is not a subtitle");
        assert_eq!(head("Re:Turn of the Ferry"), None, "too short to be a name");
        assert_eq!(
            plain("Starlit Council: Beyond the Night’s End"),
            "Starlit Council Beyond the Nights End"
        );
    }

    #[test]
    fn the_key_word_is_the_name_word_the_romaji_and_english_share() {
        let mut work = council();
        work.romaji = Some("Sorayomi no Tamsin".into());
        work.english = Some("Tamsin: Beyond the Lanterns".into());
        assert_eq!(key_word(&work).as_deref(), Some("Tamsin"));
        // Nothing shared: the romaji's longest word.
        assert_eq!(key_word(&council()).as_deref(), Some("Hoshizora"));
        work.romaji = None;
        assert_eq!(key_word(&work), None);
    }

    #[test]
    fn names_compare_as_the_same_spelt_another_way_or_missing_a_subtitle() {
        let work = council();
        assert_eq!(title(&work, "starlit council - beyond the night hours"), Title::Same);
        assert_eq!(title(&work, "Hoshizoratachi no Renai Kaigi"), Title::Spelt);
        assert_eq!(title(&work, "Starlit Council"), Title::Subtitle);
        assert_eq!(
            title(&work, "Hoshizora-tachi no Ren'ai Kaigi: Yoru no Bu"),
            Title::Subtitle
        );
        assert_eq!(title(&work, "Starlit Councils"), Title::Other);
        assert_eq!(title(&work, "Starlit Council Season 2"), Title::Other);
        assert_eq!(title(&work, "Paper Cranes"), Title::Other);
        let mut synonyms = SeriesHit::new("x", "Paper Cranes");
        synonyms.synonyms = vec!["Hoshizora-tachi no Ren'ai Kaigi".into()];
        assert_eq!(hit_title(&work, &synonyms), Title::Same, "a hit's other names count");
        let mut coloured = SeriesHit::new("x", "Starlit Council (Fan-Colored)");
        coloured.synonyms = vec!["Starlit Council: Beyond the Night Hours".into()];
        assert_eq!(
            hit_title(&work, &coloured),
            Title::Edition(Edition::Colour),
            "its own name says which edition it is"
        );
    }

    #[test]
    fn each_edition_word_marks_another_edition() {
        let work = council();
        let english = "Starlit Council: Beyond the Night Hours";
        for (words, edition) in [
            ("Full Color", Edition::Colour),
            ("(Full Colour)", Edition::Colour),
            ("Colored", Edition::Colour),
            ("Coloured", Edition::Colour),
            ("Official", Edition::Official),
            ("Digital", Edition::Digital),
            ("Novel", Edition::Novel),
            ("Light Novel", Edition::Novel),
            ("Web Novel", Edition::Novel),
            ("Anthology", Edition::Anthology),
            ("Doujinshi", Edition::Doujinshi),
            ("Spin-off", Edition::SpinOff),
            ("Spinoff", Edition::SpinOff),
            ("One-shot", Edition::OneShot),
            ("Oneshot", Edition::OneShot),
            ("Pilot", Edition::OneShot),
            ("Side Story", Edition::SideStory),
            ("Side Stories", Edition::SideStory),
            ("(Pre-serialization)", Edition::OneShot),
            ("Prototype", Edition::OneShot),
            ("(Fan-Colored)", Edition::Colour),
        ] {
            assert_eq!(
                title(&work, &format!("{english} - {words}")),
                Title::Edition(edition),
                "{words}"
            );
        }
        assert_eq!(
            title(&work, "Official Starlit Council: Beyond the Night Hours"),
            Title::Edition(Edition::Official)
        );
        assert_eq!(
            title(&work, "Hoshizora-tachi no Ren'ai Kaigi (Colored)"),
            Title::Edition(Edition::Colour)
        );
        // "Manga" marks an edition only beside a novel, and "Novel" only beside a drawn work.
        assert_ne!(title(&work, &format!("{english} (Manga)")), Title::Edition(Edition::Manga));
        let mut novel = council();
        novel.medium = Medium::Novel;
        assert_eq!(
            title(&novel, &format!("{english} (Manga)")),
            Title::Edition(Edition::Manga)
        );
        assert_ne!(
            title(&novel, &format!("{english} (Novel)")),
            Title::Edition(Edition::Novel)
        );
        // A word in the work's own name marks nothing.
        let official = named("The Official Lantern Ferry");
        assert_eq!(title(&official, "Official Lantern Ferry"), Title::Same);
        // An edition of another work is just another work.
        assert_eq!(title(&work, "Paper Cranes - Full Color"), Title::Other);
    }

    fn facts(year: Option<u32>, authors: &[&str], chapters: Option<u32>) -> Facts {
        Facts {
            format: None,
            year,
            authors: authors.iter().map(|a| (*a).to_owned()).collect(),
            chapters,
            named_one_shot: false,
            first: None,
            last: None,
        }
    }

    #[test]
    fn the_year_backs_a_match_within_one_and_otherwise_says_another_work() {
        let work = council();
        assert_eq!(fit(&work, &facts(Some(2015), &[], None)), Fit::Backed);
        assert_eq!(fit(&work, &facts(Some(2016), &[], None)), Fit::Backed);
        assert_eq!(fit(&work, &facts(Some(2012), &[], None)), Fit::Wrong);
        assert_eq!(fit(&work, &facts(None, &[], None)), Fit::Unbacked);
    }

    #[test]
    fn a_maker_backs_a_match_in_either_order_and_a_stranger_says_another_work() {
        let work = council();
        assert_eq!(fit(&work, &facts(None, &["TAMSIN Mira"], None)), Fit::Backed);
        assert_eq!(fit(&work, &facts(None, &["Oren Vale", "Mira Tamsin"], None)), Fit::Backed);
        assert_eq!(fit(&work, &facts(None, &["Oren Vale"], None)), Fit::Wrong);
        assert_eq!(fit(&work, &facts(Some(2015), &["Mira Tamsin"], None)), Fit::Sure);
    }

    #[test]
    fn the_medium_must_agree() {
        let work = council();
        let mut novel = facts(None, &[], None);
        novel.format = Some(ReadingFormat::Novel);
        assert_eq!(fit(&work, &novel), Fit::Wrong);
        let mut strip = facts(None, &[], None);
        strip.format = Some(ReadingFormat::Webtoon);
        assert_eq!(fit(&work, &strip), Fit::Unbacked);
        let mut book = council();
        book.medium = Medium::Novel;
        assert_eq!(fit(&book, &strip), Fit::Wrong);
    }

    #[test]
    fn a_chapter_count_must_be_plausible_against_the_works() {
        let work = council();
        assert_eq!(fit(&work, &facts(None, &[], Some(120))), Fit::Unbacked);
        assert_eq!(fit(&work, &facts(None, &[], Some(40))), Fit::Unbacked, "a source behind");
        assert_eq!(fit(&work, &facts(None, &[], Some(140))), Fit::Unbacked, "extras and parts");
        assert_eq!(fit(&work, &facts(None, &[], Some(400))), Fit::Wrong);
        // Extras and side chapters swell a count; the last chapter's number says what it covers.
        let mut swollen = Facts::default();
        let numbered: Vec<String> = (1..=120).map(|n| n.to_string()).collect();
        let extras: Vec<String> = (1..=90).map(|n| format!("Extra {n}")).collect();
        swollen.count_labels(
            numbered
                .iter()
                .map(|n| (Some(n.clone()), n.as_str(), None))
                .chain(extras.iter().map(|e| (None, e.as_str(), None))),
        );
        assert_eq!(swollen.chapters, Some(210));
        assert_eq!(fit(&work, &swollen), Fit::Unbacked);
        let mut longer = Facts::default();
        let far: Vec<String> = (1..=400).map(|n| n.to_string()).collect();
        longer.count_labels(far.iter().map(|n| (Some(n.clone()), n.as_str(), None)));
        assert_eq!(fit(&work, &longer), Fit::Wrong);
    }

    #[test]
    fn a_one_shot_pilot_or_single_chapter_is_never_a_serialised_work() {
        let work = council();
        assert_eq!(fit(&work, &facts(Some(2015), &["Mira Tamsin"], Some(1))), Fit::OneShot);
        let mut format = facts(None, &[], None);
        format.format = Some(ReadingFormat::OneShot);
        assert_eq!(fit(&work, &format), Fit::OneShot);
        // With no count for the work, a single chapter named a one-shot or a pilot.
        let mut releasing = council();
        releasing.chapters = None;
        let mut one = Facts::default();
        one.count_labels([(None, "Oneshot", None)].into_iter());
        assert_eq!(fit(&releasing, &one), Fit::OneShot);
        let mut pilot = Facts::default();
        pilot.count_labels([(None, "Extra", Some("Pilot chapter"))].into_iter());
        assert_eq!(fit(&releasing, &pilot), Fit::OneShot);
        let mut first = Facts::default();
        first.count_labels([(Some("1".to_owned()), "1", None)].into_iter());
        assert_eq!(fit(&releasing, &first), Fit::Unbacked, "a series one chapter in");
        // A one-shot work is one.
        let mut short = council();
        short.one_shot = true;
        short.chapters = Some(1);
        assert_eq!(fit(&short, &format), Fit::Unbacked);
    }

    #[test]
    fn counting_takes_each_chapter_once() {
        let mut counted = Facts::default();
        counted.count_labels(
            [
                (Some("1".to_owned()), "1", None),
                (Some("1".to_owned()), "Chapter 1", None),
                (Some("2".to_owned()), "2", None),
                (None, "Extra", None),
            ]
            .into_iter(),
        );
        assert_eq!(counted.chapters, Some(3));
        assert!(!counted.named_one_shot);
    }

    #[test]
    fn a_near_name_is_taken_only_with_what_backs_it() {
        assert!(taken(Title::Same, Fit::Unbacked));
        assert!(!taken(Title::Same, Fit::OneShot));
        assert!(!taken(Title::Same, Fit::Wrong));
        assert!(taken(Title::Spelt, Fit::Backed));
        assert!(!taken(Title::Spelt, Fit::Unbacked));
        assert!(taken(Title::Subtitle, Fit::Sure));
        assert!(!taken(Title::Subtitle, Fit::Backed));
        assert!(!taken(Title::Edition(Edition::Colour), Fit::Sure));
        assert!(!taken(Title::Other, Fit::Sure));
    }

    #[test]
    fn makers_are_who_wrote_or_drew_it() {
        assert!(is_maker("Story & Art"));
        assert!(is_maker("Original Creator"));
        assert!(!is_maker("Translator (English)"));
        assert!(!is_maker("Touch-up Art & Lettering"));
        assert!(!is_maker("Assistant"));
    }
}
