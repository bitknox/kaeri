//! Media vocabulary: episodes, search hits, and the sub/dub axis.

use std::{cmp::Ordering, fmt, str::FromStr, time::Duration};

use serde::{Deserialize, Serialize};

/// Whether we want subtitled or dubbed audio.
///
/// Providers treat these as genuinely different catalogues — a title can have 28 subbed
/// episodes and 12 dubbed ones — so this is threaded through search, episode listing and
/// resolution rather than applied as a filter at the end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
#[serde(rename_all = "lowercase")]
pub enum Translation {
    #[default]
    Sub,
    Dub,
}

impl Translation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sub => "sub",
            Self::Dub => "dub",
        }
    }

    /// The other one. Used for the `t` keybinding and for fallback when a provider has
    /// only one of the two.
    pub const fn toggled(self) -> Self {
        match self {
            Self::Sub => Self::Dub,
            Self::Dub => Self::Sub,
        }
    }
}

impl fmt::Display for Translation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Translation {
    type Err = ParseTranslationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "sub" | "subbed" | "subtitled" => Ok(Self::Sub),
            "dub" | "dubbed" => Ok(Self::Dub),
            _ => Err(ParseTranslationError(s.to_owned())),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("not a translation type: {0:?} (expected \"sub\" or \"dub\")")]
pub struct ParseTranslationError(String);

/// An episode identifier as the *provider* expresses it.
///
/// Kept as a string rather than a number on purpose. Real catalogues contain `"12"`,
/// `"12.5"` (recap and interlude episodes), `"OVA"`, and `"S1"`. Forcing these into an
/// integer either loses episodes or invents ones that don't exist, so the raw label is
/// preserved and a numeric view is offered alongside it for sorting and comparison.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EpisodeNumber(String);

impl EpisodeNumber {
    pub fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Numeric value, when the label is numeric. `None` for `"OVA"` and friends.
    pub fn as_number(&self) -> Option<f64> {
        self.0.trim().parse::<f64>().ok()
    }

    /// Zero-padded to a fixed three-column field for the timing-sheet episode table:
    /// `9` renders as `009`, `12.5` as `012.5`, and non-numeric labels pass through.
    ///
    /// The fixed field is what makes the column scan cleanly; ragged numerals are the
    /// fastest way to make a dense table look accidental.
    pub fn padded(&self) -> String {
        match self.as_number() {
            Some(n) => {
                let whole = n.trunc() as i64;
                let frac = self.0.trim().split_once('.').map(|(_, f)| f.to_owned());
                match frac {
                    Some(f) => format!("{whole:03}.{f}"),
                    None => format!("{whole:03}"),
                }
            }
            None => self.0.clone(),
        }
    }
}

impl fmt::Display for EpisodeNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for EpisodeNumber {
    fn from(raw: &str) -> Self {
        Self(raw.to_owned())
    }
}

impl From<u32> for EpisodeNumber {
    fn from(raw: u32) -> Self {
        Self(raw.to_string())
    }
}

impl PartialOrd for EpisodeNumber {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EpisodeNumber {
    /// Numeric episodes sort numerically and before non-numeric ones; specials and OVAs
    /// sort lexically at the end. A naive string sort would put episode 10 before 9.
    ///
    /// **Equal ordering means equal value.** `Eq` is derived on the label, so answering
    /// `Equal` for `"7"` and `"07"`, which numeric comparison alone does, would break the
    /// contract every ordered collection relies on: `sort` then `dedup` kept both,
    /// `binary_search` reported finding one when it had found the other, and a `BTreeMap`
    /// could end up holding one insertion's key beside another's value. Catalogues do pad
    /// inconsistently, so this is reachable rather than theoretical. The label breaks the
    /// tie, which keeps numeric ordering and makes the two agree.
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.as_number(), other.as_number()) {
            (Some(a), Some(b)) => {
                a.partial_cmp(&b).unwrap_or(Ordering::Equal).then_with(|| self.0.cmp(&other.0))
            }
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => self.0.cmp(&other.0),
        }
    }
}

/// A single episode as listed by a provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Episode {
    pub number: EpisodeNumber,
    pub title: Option<String>,
    pub duration: Option<Duration>,
    /// Still frame for this episode, when something knows one.
    ///
    /// On the type rather than bolted onto the UI row because it is a property of the episode:
    /// a remote provider can publish one, and so can the metadata layer for a source that has
    /// no catalogue of its own.
    pub thumbnail: Option<String>,
    /// One-paragraph synopsis, when the source has one. Plain text.
    #[serde(default)]
    pub description: Option<String>,
    /// First-air date as `YYYY-MM-DD`, when known.
    #[serde(default)]
    pub air_date: Option<String>,
    /// The source's own filler claim. `None` is "no claim", distinct from `Some(false)` —
    /// a positive claim of canon. A fallback where the filler index does not cover the show.
    #[serde(default)]
    pub filler: Option<bool>,
}

impl Episode {
    pub fn new(number: impl Into<EpisodeNumber>) -> Self {
        Self {
            number: number.into(),
            title: None,
            duration: None,
            thumbnail: None,
            description: None,
            air_date: None,
            filler: None,
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_thumbnail(mut self, url: impl Into<String>) -> Self {
        self.thumbnail = Some(url.into());
        self
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }
}

/// Broad shape of a title. Used as a *match gate* during provider resolution: this is
/// what stops a search for a TV series landing on its OVA or a recap movie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MediaFormat {
    Tv,
    TvShort,
    Movie,
    Special,
    Ova,
    Ona,
    Music,
    /// Anything this enum does not model — including the manga, novel and one-shot
    /// formats that ride along on relation edges. A related *source material* must not
    /// make the whole title fail to parse, which is exactly what `unknown variant
    /// MANGA` did the moment relations were requested.
    #[serde(other)]
    Unknown,
}

impl MediaFormat {
    /// Whether two formats are close enough to be the same work.
    ///
    /// Intentionally loose across the TV variants and strict about everything else,
    /// because provider catalogues disagree constantly about `TV` vs `ONA` while an
    /// `OVA`/`TV` mismatch is nearly always a genuinely wrong match.
    pub fn compatible_with(self, other: Self) -> bool {
        use MediaFormat::{Ona, Tv, TvShort, Unknown};
        if self == other || self == Unknown || other == Unknown {
            return true;
        }
        // Catalogues disagree constantly about TV vs TV_SHORT vs ONA for the same work,
        // so those are treated as one family. Everything else must match exactly.
        matches!((self, other), (Tv | TvShort | Ona, Tv | TvShort | Ona))
    }
}

/// Airing status of a title.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MediaStatus {
    Finished,
    Releasing,
    NotYetReleased,
    Cancelled,
    Hiatus,
    /// A status AniList adds later, read as this rather than failing the title over it.
    #[serde(other)]
    Unknown,
}

/// What a work is: a show, or a book of pictures or of words. A light novel is [`Medium::Novel`];
/// manhwa and manhua are [`Medium::Manga`] with an [`Origin`], and a webtoon is a
/// [`ReadingFormat`]. One work is one series in one medium.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
#[serde(rename_all = "lowercase")]
pub enum Medium {
    Anime,
    Manga,
    Novel,
}

impl Medium {
    /// The name the store, sync and plugins use: `anime`, `manga`, `novel`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Anime => "anime",
            Self::Manga => "manga",
            Self::Novel => "novel",
        }
    }

    /// The medium a plugin or the store names, `None` for anything else. AniList's own words
    /// (`ANIME`, `MANGA`, a `NOVEL` format, `light novel`) are read too.
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().replace(['_', '-'], " ").as_str() {
            "anime" | "show" | "shows" => Some(Self::Anime),
            "manga" | "manhwa" | "manhua" | "comic" | "comics" | "webtoon" => Some(Self::Manga),
            "novel" | "novels" | "light novel" | "web novel" | "ln" => Some(Self::Novel),
            _ => None,
        }
    }

    /// Whether this is a medium that is read rather than watched.
    pub const fn is_reading(self) -> bool {
        matches!(self, Self::Manga | Self::Novel)
    }
}

impl fmt::Display for Medium {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How a book is laid out. Plugins and servers name it as a string; one this does not model is
/// kept as [`ReadingFormat::Other`], since a person may see it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadingFormat {
    Manga,
    OneShot,
    /// One long strip read from top to bottom.
    Webtoon,
    Novel,
    Other(String),
}

impl ReadingFormat {
    /// A plugin's or AniList's name for a format: `manga`, `one-shot`, `ONE_SHOT`, `webtoon`,
    /// `novel`, `NOVEL`. An empty name is `None`.
    pub fn parse(name: &str) -> Option<Self> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return None;
        }
        Some(match trimmed.to_ascii_lowercase().replace(['_', '-', ' '], "").as_str() {
            "manga" | "manhwa" | "manhua" | "comic" => Self::Manga,
            "oneshot" => Self::OneShot,
            "webtoon" | "strip" | "longstrip" => Self::Webtoon,
            "novel" | "lightnovel" | "webnovel" => Self::Novel,
            _ => Self::Other(trimmed.to_owned()),
        })
    }

    /// The name plugins and the store use for it; an unknown format keeps its own.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Manga => "manga",
            Self::OneShot => "one-shot",
            Self::Webtoon => "webtoon",
            Self::Novel => "novel",
            Self::Other(name) => name,
        }
    }

    /// The medium a book of this format belongs to, where the format says.
    pub fn medium(&self) -> Option<Medium> {
        match self {
            Self::Manga | Self::OneShot | Self::Webtoon => Some(Medium::Manga),
            Self::Novel => Some(Medium::Novel),
            Self::Other(_) => None,
        }
    }
}

/// Where a book comes from, by country. Manhwa is [`Medium::Manga`] from [`Origin::Korea`].
/// Plugins send an ISO 3166 code and AniList its `countryOfOrigin`; another country is kept as
/// [`Origin::Other`] with its code.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    Japan,
    Korea,
    China,
    Taiwan,
    Other(String),
}

impl Origin {
    /// An ISO 3166 code (`JP`, `KR`, `CN`, `TW`) or a country's English name. Empty is `None`.
    pub fn parse(code: &str) -> Option<Self> {
        let trimmed = code.trim();
        if trimmed.is_empty() {
            return None;
        }
        Some(match trimmed.to_ascii_lowercase().as_str() {
            "jp" | "jpn" | "japan" => Self::Japan,
            "kr" | "kor" | "korea" | "south korea" => Self::Korea,
            "cn" | "chn" | "china" => Self::China,
            "tw" | "twn" | "taiwan" => Self::Taiwan,
            _ => Self::Other(trimmed.to_ascii_uppercase()),
        })
    }

    /// The ISO 3166 code: `JP`, `KR`, `CN`, `TW`, or the one kept.
    pub fn code(&self) -> &str {
        match self {
            Self::Japan => "JP",
            Self::Korea => "KR",
            Self::China => "CN",
            Self::Taiwan => "TW",
            Self::Other(code) => code,
        }
    }
}

/// What is known about the title being asked for, handed to a source with each search, episode
/// listing and stream request. Everything is optional: a source may use any of it or none.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TitleHints {
    pub anilist_id: Option<crate::ids::AnilistId>,
    pub romaji: Option<String>,
    pub english: Option<String>,
    pub native: Option<String>,
    pub synonyms: Vec<String>,
}

impl TitleHints {
    /// The same hints with the AniList id set, for a caller that knows the id but built the names
    /// elsewhere.
    pub fn with_anilist(mut self, id: crate::ids::AnilistId) -> Self {
        self.anilist_id = Some(id);
        self
    }
}

/// One candidate returned by a provider's search.
///
/// The extra fields beyond `title` exist for scoring, not display: episode count, year
/// and format are the gates that turn a fuzzy title match into a confident one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    pub key: crate::ids::ProviderKey,
    pub title: String,
    /// Alternate titles the provider offered, if any. Widens the match surface.
    #[serde(default)]
    pub synonyms: Vec<String>,
    pub episode_count: Option<u32>,
    pub year: Option<u16>,
    pub format: Option<MediaFormat>,
}

impl SearchHit {
    pub fn new(key: impl Into<crate::ids::ProviderKey>, title: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            synonyms: Vec::new(),
            episode_count: None,
            year: None,
            format: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_formats_and_origins_read_what_plugins_and_anilist_send() {
        assert_eq!(Medium::parse("MANGA"), Some(Medium::Manga));
        assert_eq!(Medium::parse("light_novel"), Some(Medium::Novel));
        assert_eq!(Medium::parse("podcast"), None);
        assert_eq!(ReadingFormat::parse("ONE_SHOT"), Some(ReadingFormat::OneShot));
        assert_eq!(ReadingFormat::parse("one-shot"), Some(ReadingFormat::OneShot));
        assert_eq!(
            ReadingFormat::parse("Webtoon").and_then(|f| f.medium()),
            Some(Medium::Manga)
        );
        assert_eq!(ReadingFormat::parse("4-koma"), Some(ReadingFormat::Other("4-koma".into())));
        assert_eq!(ReadingFormat::parse(" "), None);
        assert_eq!(Origin::parse("kr"), Some(Origin::Korea));
        assert_eq!(Origin::parse("fr").map(|o| o.code().to_owned()), Some("FR".into()));
        assert_eq!(Origin::Taiwan.code(), "TW");
    }

    #[test]
    fn translation_round_trips_and_toggles() {
        assert_eq!("Dubbed".parse::<Translation>().unwrap(), Translation::Dub);
        assert_eq!("sub".parse::<Translation>().unwrap(), Translation::Sub);
        assert_eq!(Translation::Sub.toggled(), Translation::Dub);
        assert!("swedish".parse::<Translation>().is_err());
    }

    #[test]
    fn episodes_sort_numerically_not_lexically() {
        let mut eps: Vec<EpisodeNumber> =
            ["10", "9", "12.5", "1", "OVA", "2"].into_iter().map(EpisodeNumber::from).collect();
        eps.sort();
        let got: Vec<&str> = eps.iter().map(EpisodeNumber::as_str).collect();
        // A plain string sort would yield 1, 10, 12.5, 2, 9, OVA.
        assert_eq!(got, ["1", "2", "9", "10", "12.5", "OVA"]);
    }

    #[test]
    fn ordering_agrees_with_equality() {
        // The contract every ordered collection assumes. Comparing numerically alone called
        // "07" and "7" equal while `Eq` called them different, so `sort` + `dedup` kept both
        // and `binary_search` would report finding one having found the other. Catalogues pad
        // inconsistently, so a list can genuinely hold both spellings.
        let padded = EpisodeNumber::from("07");
        let bare = EpisodeNumber::from("7");
        assert_ne!(padded, bare, "different labels");
        assert_ne!(padded.cmp(&bare), Ordering::Equal, "so they must not compare equal");

        // Numeric ordering still decides everything else.
        assert_eq!(EpisodeNumber::from("9").cmp(&EpisodeNumber::from("10")), Ordering::Less);
        assert_eq!(EpisodeNumber::from("7").cmp(&EpisodeNumber::from("7")), Ordering::Equal);

        let mut eps: Vec<EpisodeNumber> =
            ["07", "7", "8"].into_iter().map(EpisodeNumber::from).collect();
        eps.sort();
        eps.dedup();
        assert_eq!(eps.len(), 3, "dedup must not collapse labels that are not equal");
    }

    #[test]
    fn padding_produces_a_fixed_width_field() {
        assert_eq!(EpisodeNumber::from("9").padded(), "009");
        assert_eq!(EpisodeNumber::from("012").padded(), "012");
        assert_eq!(EpisodeNumber::from("12.5").padded(), "012.5");
        assert_eq!(EpisodeNumber::from("OVA").padded(), "OVA");
    }

    #[test]
    fn format_gate_rejects_ova_for_tv_but_tolerates_tv_ona_disagreement() {
        assert!(MediaFormat::Tv.compatible_with(MediaFormat::Ona));
        assert!(MediaFormat::Tv.compatible_with(MediaFormat::TvShort));
        assert!(MediaFormat::Tv.compatible_with(MediaFormat::Unknown));
        // The case that matters: an OVA is not the TV series.
        assert!(!MediaFormat::Tv.compatible_with(MediaFormat::Ova));
        assert!(!MediaFormat::Tv.compatible_with(MediaFormat::Movie));
    }
}
