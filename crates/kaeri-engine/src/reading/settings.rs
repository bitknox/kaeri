//! The reader's settings and where each value comes from.
//!
//! A value is looked for in layers: the series' own choice (synced, or this device only), then
//! what its source and the catalogue say of how it reads (the mode and direction of a comic),
//! then the reader's guess, then the medium's value in `config.toml` (`[reading.manga]`,
//! `[reading.novel]`; the choices among them sync as shared keys), then the default. The e-ink
//! refresh settings are the device's, in `[reading.eink]`, whatever the medium, and so are the
//! tap zones, in `[reading.taps]`.

use std::collections::BTreeMap;

use kaeri_core::{
    books::ReadingHint, config::ReadingConfig, media::Medium, settings::SettingValue,
};
use kaeri_model::{
    eink::{EinkMatch, EinkPanel, EinkProfile},
    reader::{ReaderSettingKey as Key, ReaderSettingRow, ReaderValue, SettingScope},
    tap_zones::{GuideKind, TapZoneSettings, ZoneLayout, ZoneSize},
};
use kaeri_store::{Store, reading::SeriesSetting};

/// What the settings sheet is shown on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Screen {
    /// The panel, on an e-ink screen.
    pub eink: Option<EinkPanel>,
    /// The family's refresh hook resolved: the Refresh rows show.
    pub refresh_hook: bool,
    /// A phone or a tablet.
    pub handheld: bool,
    /// A phone, not a tablet.
    pub phone: bool,
}

impl Screen {
    pub fn new(profile: Option<&EinkProfile>, handheld: bool, phone: bool) -> Self {
        let profile = profile.filter(|p| p.eink != EinkMatch::No);
        Self {
            eink: profile.map(|p| p.panel),
            refresh_hook: profile.is_some_and(|p| p.refresh_hook),
            handheld,
            phone,
        }
    }

    fn colour_eink(self) -> bool {
        self.eink.is_some_and(|p| p.colour() && p != EinkPanel::Lcd)
    }
}

fn choice(name: &str) -> ReaderValue {
    ReaderValue::Choice { name: name.to_owned() }
}

fn choices(names: &[&str]) -> Vec<ReaderValue> {
    names.iter().map(|n| choice(n)).collect()
}

/// The choices a setting offers; empty for amounts and toggles.
pub fn choices_for(key: Key, medium: Medium) -> Vec<ReaderValue> {
    match key {
        Key::Mode if medium == Medium::Novel => choices(&["paged", "scrolled"]),
        Key::Mode => choices(&["paged", "vertical", "strip"]),
        Key::Direction => choices(&["rtl", "ltr"]),
        Key::Colour => choices(&["auto", "colour", "greys"]),
        Key::Look => choices(&["system", "paper", "light", "sepia", "dark"]),
        Key::Font => choices(&["serif", "sans"]),
        Key::Fit => choices(&["screen", "width", "height"]),
        Key::Background => choices(&["theme", "black", "white"]),
        Key::PageLayout => choices(&["auto", "single", "double"]),
        Key::Spreads => choices(&["split", "rotate", "whole"]),
        Key::TapLayout => choices(&["edges", "sides", "off"]),
        Key::TapPaged => ZoneLayout::PAGED.iter().map(|l| choice(l.name())).collect(),
        Key::TapScroll => ZoneLayout::SCROLLING.iter().map(|l| choice(l.name())).collect(),
        Key::TapSize => choices(&[ZoneSize::Standard.name(), ZoneSize::Small.name()]),
        Key::GallerySpeed => choices(&["standard", "fast", "best"]),
        Key::FootValue => choices(&["pages_left", "none"]),
        Key::StripGap => choices(&["none", "small"]),
        _ => Vec::new(),
    }
}

/// A setting's default for `medium` on `screen`.
pub fn default_for(key: Key, medium: Medium, screen: Screen) -> ReaderValue {
    let on = |on| ReaderValue::Toggle { on };
    let amount = |value| ReaderValue::Amount { value };
    match key {
        Key::Mode => choice("paged"),
        Key::Direction => choice("rtl"),
        Key::Colour => choice("auto"),
        Key::Look => choice(if screen.eink.is_some() { "paper" } else { "light" }),
        Key::Font => choice("serif"),
        Key::CropBorders => on(screen.eink.is_some()),
        Key::Fit => choice("screen"),
        Key::Background => choice("theme"),
        Key::PageLayout => choice("auto"),
        Key::Spreads => choice("split"),
        Key::TextSize | Key::Margins | Key::Contrast => amount(1.0),
        Key::LineSpacing => {
            amount(if medium == Medium::Novel && screen.phone { 1.6 } else { 1.4 })
        }
        Key::Justify => on(medium == Medium::Novel && (screen.eink.is_some() || !screen.phone)),
        Key::TapLayout => choice("edges"),
        // E-ink and the desktop start on Sides; a phone or a tablet on Auto, so vertical paging
        // gets zones that point down.
        Key::TapPaged if screen.eink.is_some() || !screen.handheld => {
            choice(ZoneLayout::Sides.name())
        }
        Key::TapPaged => choice(ZoneLayout::Auto.name()),
        Key::TapScroll => choice(ZoneLayout::Off.name()),
        Key::TapSize => choice(ZoneSize::Standard.name()),
        Key::TapFlipSides | Key::TapFlipEnds => on(false),
        Key::SwipeTurns => on(true),
        Key::VolumeKeysTurn => on(screen.eink.is_some()),
        Key::StripWidth => amount(0.6),
        Key::RefreshImage => on(true),
        Key::RefreshText => amount(6.0),
        Key::RefreshList => {
            amount(if screen.eink == Some(EinkPanel::Kaleido3) { 3.0 } else { 5.0 })
        }
        Key::GallerySpeed => choice("standard"),
        Key::RainbowFilter | Key::PaperMode | Key::GreyscalePages | Key::QuietPhone => {
            on(false)
        }
        Key::Saturation => {
            amount(if screen.eink == Some(EinkPanel::Kaleido3) { 1.2 } else { 1.0 })
        }
        Key::FootValue => choice("pages_left"),
        Key::Folio | Key::SkipCreditPages => on(true),
        Key::StripGap => choice("none"),
        // Paper mode's parts, on by default inside it; the page's white dimmed a little.
        Key::PaperInstant | Key::PaperLook | Key::PaperVolume => on(true),
        Key::PaperDim => amount(0.3),
    }
}

/// Whether the sheet shows the setting on this screen.
pub fn shown(key: Key, medium: Medium, screen: Screen) -> bool {
    if !key.serves(medium) {
        return false;
    }
    match key {
        Key::RefreshImage | Key::RefreshText | Key::RefreshList => screen.refresh_hook,
        Key::GallerySpeed => screen.eink == Some(EinkPanel::Gallery3),
        Key::RainbowFilter => screen.eink == Some(EinkPanel::Kaleido3),
        Key::Saturation | Key::Contrast => screen.colour_eink(),
        Key::PaperMode
        | Key::GreyscalePages
        | Key::QuietPhone
        | Key::PaperInstant
        | Key::PaperLook
        | Key::PaperDim
        | Key::PaperVolume => screen.handheld && screen.eink.is_none(),
        Key::StripGap => screen.eink.is_none(),
        Key::Colour => screen.eink != Some(EinkPanel::Grey),
        Key::Background => screen.eink.is_none(),
        Key::StripWidth => !screen.handheld,
        Key::VolumeKeysTurn => screen.handheld || screen.eink.is_some(),
        // E-ink has one tap zone choice for all reading, and only e-ink has the swipe switch.
        Key::TapScroll => screen.eink.is_none(),
        Key::SwipeTurns => screen.eink.is_some(),
        Key::TapLayout => false,
        _ => true,
    }
}

/// The config table of the tap zones and their guides.
pub const TAPS: [&str; 2] = ["reading", "taps"];

/// The config table a setting's medium value is kept in.
pub fn table(key: Key, medium: Medium) -> [&'static str; 2] {
    match key {
        Key::RefreshImage | Key::RefreshText | Key::RefreshList | Key::GallerySpeed => {
            ["reading", "eink"]
        }
        Key::TapPaged
        | Key::TapScroll
        | Key::TapSize
        | Key::TapFlipSides
        | Key::TapFlipEnds
        | Key::SwipeTurns => TAPS,
        _ if medium == Medium::Novel => ["reading", "novel"],
        _ => ["reading", "manga"],
    }
}

fn map(config: &ReadingConfig, key: Key, medium: Medium) -> &BTreeMap<String, toml::Value> {
    match table(key, medium)[1] {
        "eink" => &config.eink,
        "taps" => &config.taps,
        "novel" => &config.novel,
        _ => &config.manga,
    }
}

fn from_toml(value: &toml::Value) -> Option<ReaderValue> {
    Some(match value {
        toml::Value::String(name) => ReaderValue::Choice { name: name.clone() },
        toml::Value::Boolean(on) => ReaderValue::Toggle { on: *on },
        toml::Value::Integer(n) => ReaderValue::Amount { value: *n as f64 },
        toml::Value::Float(f) => ReaderValue::Amount { value: *f },
        _ => return None,
    })
}

pub fn to_setting(value: &ReaderValue) -> SettingValue {
    match value {
        ReaderValue::Choice { name } => SettingValue::Str(name.clone()),
        ReaderValue::Amount { value } => SettingValue::Float(*value),
        ReaderValue::Toggle { on } => SettingValue::Bool(*on),
    }
}

/// What a series' source and the catalogue say of how its pages read. It comes after the
/// person's own choice for the series and before the reader's guess from the book.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SourceWord {
    /// The source's reading hint.
    pub hint: Option<ReadingHint>,
    /// The catalogue counts the series a webtoon: tagged a long strip, or from Korea, China or
    /// Taiwan.
    pub webtoon: bool,
}

impl SourceWord {
    /// What the series' source and the catalogue say of how `work` reads, as last kept.
    pub(crate) fn of(
        services: &crate::services::Services,
        work: &kaeri_core::books::WorkId,
    ) -> Self {
        let kind = services.store.book_work_kind(work.as_str()).ok().flatten();
        // A demo work says it itself: opened before its page was ever loaded, its source was
        // never asked.
        let demo = kaeri_core::reading_demo::WORKS.iter().find(|w| w.work_id() == *work);
        Self {
            hint: services.book_sources.hint_of(work).or_else(|| demo?.reading),
            webtoon: kind.as_ref().is_some_and(kaeri_store::shelf::BookWorkKind::webtoon),
        }
    }
}

/// Everything the layers say, read once for a sheet or a session.
#[derive(Debug, Clone)]
pub struct Layers {
    pub medium: Medium,
    pub screen: Screen,
    series: Vec<SeriesSetting>,
    source: SourceWord,
    config: ReadingConfig,
}

impl Layers {
    pub fn read(
        store: &Store,
        config: &ReadingConfig,
        medium: Medium,
        work: Option<&str>,
        screen: Screen,
    ) -> Self {
        let series = work
            .map(|w| {
                store.series_settings(w).unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "the series' reader settings were not read");
                    Vec::new()
                })
            })
            .unwrap_or_default();
        Self { medium, screen, series, source: SourceWord::default(), config: config.clone() }
    }

    /// The layers with what the series' source and the catalogue say of it.
    pub fn with_source(mut self, source: SourceWord) -> Self {
        self.source = source;
        self
    }

    /// The source's or the catalogue's value for `key`: a comic's mode and direction only. A
    /// strip, said by either, opens as the Webtoons default. Left to right and right to left are
    /// a paged book, as in Komga, where they stand beside vertical and webtoon, unless the
    /// catalogue counts the series a webtoon.
    fn source_value(&self, key: Key) -> Option<ReaderValue> {
        if self.medium != Medium::Manga {
            return None;
        }
        let hint = self.source.hint;
        match key {
            Key::Mode => match hint {
                Some(ReadingHint::Vertical) => Some(choice("vertical")),
                Some(ReadingHint::Strip) => Some(choice(self.config.webtoon.mode.name())),
                _ if self.source.webtoon => Some(choice(self.config.webtoon.mode.name())),
                Some(ReadingHint::RightToLeft | ReadingHint::LeftToRight) => {
                    Some(choice("paged"))
                }
                None => None,
            },
            Key::Direction => match hint? {
                ReadingHint::RightToLeft => Some(choice("rtl")),
                ReadingHint::LeftToRight => Some(choice("ltr")),
                ReadingHint::Vertical | ReadingHint::Strip => None,
            },
            _ => None,
        }
    }

    /// One setting's row: its value and where the value comes from.
    pub fn row(&self, key: Key) -> ReaderSettingRow {
        let series = key
            .series_scope()
            .and_then(|_| self.series.iter().find(|s| s.setting == key.name()));
        let chosen = series.filter(|s| s.scope != "auto");
        let medium = self.kept(key);
        let source = if chosen.is_none() { self.source_value(key) } else { None };
        let (value, scope, detected, overridden) = match (chosen, source, series, medium) {
            (Some(s), ..) => {
                let scope = if s.scope == "series" {
                    SettingScope::Series
                } else {
                    SettingScope::SeriesHere
                };
                (ReaderValue::decode(&s.value), scope, false, true)
            }
            // Said by the series' details: "Detected from the series details".
            (None, Some(value), ..) => (value, SettingScope::Series, true, false),
            (None, None, Some(s), _) => {
                (ReaderValue::decode(&s.value), SettingScope::Series, true, false)
            }
            (None, None, None, Some(value)) => (value, key.home_scope(), false, false),
            (None, None, None, None) => {
                (default_for(key, self.medium, self.screen), key.home_scope(), false, false)
            }
        };
        ReaderSettingRow {
            key,
            value,
            scope,
            detected,
            overridden,
            choices: choices_for(key, self.medium),
        }
    }

    /// The device's or the medium's value of `key` in the config, unset as `None`. A tap
    /// layout reads as its current name, Edges and Halves as Sides, and an unknown one as
    /// unset; unset, the paged layout takes the earlier `tap_layout` of the open medium, then
    /// of the other one.
    fn kept(&self, key: Key) -> Option<ReaderValue> {
        let value = map(&self.config, key, self.medium).get(key.name()).and_then(from_toml);
        match key {
            Key::TapPaged | Key::TapScroll => {
                let layout = |v: ReaderValue| match v {
                    ReaderValue::Choice { name } => ZoneLayout::parse(&name),
                    _ => None,
                };
                let kept = value.and_then(layout).filter(|l| {
                    key == Key::TapPaged || ZoneLayout::SCROLLING.contains(l)
                });
                let earlier = || {
                    let (first, second) = match self.medium {
                        Medium::Novel => (&self.config.novel, &self.config.manga),
                        _ => (&self.config.manga, &self.config.novel),
                    };
                    let name = Key::TapLayout.name();
                    let old = first.get(name).or_else(|| second.get(name))?;
                    Some(ZoneLayout::from_earlier(old.as_str().unwrap_or_default()))
                };
                let kept = if key == Key::TapPaged { kept.or_else(earlier) } else { kept };
                kept.map(|l| choice(l.name()))
            }
            Key::TapSize => value.filter(|v| {
                matches!(v, ReaderValue::Choice { name } if ZoneSize::parse(name).is_some())
            }),
            _ => value,
        }
    }

    /// The tap zone settings in effect on this screen, with the guides seen.
    pub fn tap_zones(&self) -> TapZoneSettings {
        let layout = |key| ZoneLayout::parse(&self.word(key)).unwrap_or_default();
        let seen = |kind: GuideKind| {
            self.config
                .taps
                .get(guide_key(kind))
                .and_then(toml::Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        TapZoneSettings {
            paged: layout(Key::TapPaged),
            scrolling: layout(Key::TapScroll),
            size: ZoneSize::parse(&self.word(Key::TapSize)).unwrap_or_default(),
            flip_sides: self.on(Key::TapFlipSides),
            flip_ends: self.on(Key::TapFlipEnds),
            eink: self.screen.eink.is_some(),
            swipes_turn: self.on(Key::SwipeTurns),
            guide_seen_paged: seen(GuideKind::Paged),
            guide_seen_scrolling: seen(GuideKind::Scrolling),
        }
    }

    pub fn value(&self, key: Key) -> ReaderValue {
        self.row(key).value
    }

    pub fn word(&self, key: Key) -> String {
        match self.value(key) {
            ReaderValue::Choice { name } => name,
            other => other.encode(),
        }
    }

    pub fn on(&self, key: Key) -> bool {
        matches!(self.value(key), ReaderValue::Toggle { on: true })
    }

    pub fn amount(&self, key: Key) -> f64 {
        match (self.value(key), default_for(key, self.medium, self.screen)) {
            (ReaderValue::Amount { value }, _) | (_, ReaderValue::Amount { value }) => value,
            _ => 0.0,
        }
    }

    /// Whether the series has a value of its own, chosen or guessed.
    pub fn series_has(&self, key: Key) -> bool {
        self.series.iter().any(|s| s.setting == key.name())
    }

    /// Whether the value comes from the series' source or the catalogue, not the person or the
    /// reader's guess: the reader then guesses nothing.
    pub fn sourced(&self, key: Key) -> bool {
        let chosen = self.series.iter().any(|s| s.setting == key.name() && s.scope != "auto");
        !chosen && self.source_value(key).is_some()
    }

    /// The rows the sheet shows on this screen.
    pub fn rows(&self) -> Vec<ReaderSettingRow> {
        Key::ALL
            .iter()
            .copied()
            .filter(|key| shown(*key, self.medium, self.screen))
            .map(|key| self.row(key))
            .collect()
    }
}

/// Why a setting was not kept.
#[derive(Debug, thiserror::Error)]
pub enum SetError {
    #[error("{0:?} is not a setting a series sets")]
    NotPerSeries(Key),
    #[error("a series setting names no work")]
    NoWork,
    #[error(transparent)]
    Store(#[from] kaeri_store::StoreError),
    #[error(transparent)]
    Config(#[from] kaeri_core::Error),
}

/// Keep a setting at `scope`. A series' choice follows the person when the scope is
/// [`SettingScope::Series`] and its follow switch is on: mode and direction follow with
/// `reading.mode_follows`, the look (colour) with `reading.look_follows`.
#[allow(clippy::too_many_arguments)]
pub fn set(
    config: &mut crate::config::ConfigState,
    paths: &kaeri_core::config::Paths,
    store: &Store,
    medium: Medium,
    work: Option<&str>,
    key: Key,
    scope: SettingScope,
    value: &ReaderValue,
    at: i64,
) -> Result<(), SetError> {
    match scope {
        SettingScope::Series | SettingScope::SeriesHere => {
            if key.series_scope().is_none() {
                return Err(SetError::NotPerSeries(key));
            }
            let work = work.ok_or(SetError::NoWork)?;
            let follow = scope == SettingScope::Series
                && key.series_scope() == Some(SettingScope::Series)
                && follows(&config.effective().reading, key);
            store.set_series_setting(work, key.name(), Some(&value.encode()), follow, at)?;
        }
        SettingScope::Medium | SettingScope::Device => {
            config.write(paths, &table(key, medium), key.name(), to_setting(value))?;
            // The open reader reads the settings in effect: the file's now.
            let reading = config.saved().reading.clone();
            config.effective_mut().reading = reading;
        }
    }
    Ok(())
}

/// Whether a series' own value of `key` follows the person: its follow switch.
pub fn follows(reading: &ReadingConfig, key: Key) -> bool {
    match key {
        Key::Mode | Key::Direction => reading.mode_follows(),
        _ => reading.look_follows(),
    }
}

/// The config key under [`TAPS`] that keeps the guide last seen of `kind`.
pub const fn guide_key(kind: GuideKind) -> &'static str {
    match kind {
        GuideKind::Paged => "guide_paged",
        GuideKind::Scrolling => "guide_scrolling",
    }
}

/// Keep the tap zone guide `guide` as seen, so it shows again only once the zones change. A
/// value that names no kind is dropped.
pub fn guide_seen(
    config: &mut crate::config::ConfigState,
    paths: &kaeri_core::config::Paths,
    guide: &str,
) -> Result<(), SetError> {
    let Some(kind) = GuideKind::of(guide) else { return Ok(()) };
    config.write(paths, &TAPS, guide_key(kind), SettingValue::Str(guide.to_owned()))?;
    let reading = config.saved().reading.clone();
    config.effective_mut().reading = reading;
    Ok(())
}

/// Drop the series' own value, so the medium's applies again.
pub fn use_default(store: &Store, work: &str, key: Key, at: i64) -> Result<(), SetError> {
    store.set_series_setting(work, key.name(), None, false, at)?;
    Ok(())
}

/// Settings whose change lays the pages out again, rather than only redrawing them.
pub const fn relays(key: Key) -> bool {
    matches!(
        key,
        Key::Mode
            | Key::Direction
            | Key::Colour
            | Key::Look
            | Key::CropBorders
            | Key::Fit
            | Key::PageLayout
            | Key::Spreads
            | Key::StripWidth
            | Key::RainbowFilter
            | Key::Saturation
            | Key::Contrast
            | Key::GreyscalePages
            | Key::SkipCreditPages
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_series_choice_beats_the_medium_and_a_guess_says_so() {
        let store = Store::open_in_memory().unwrap();
        let mut config = ReadingConfig::default();
        config.manga.insert("direction".into(), toml::Value::String("ltr".into()));
        let work = "a:900401";
        let screen = Screen::default();
        let layers = Layers::read(&store, &config, Medium::Manga, Some(work), screen);
        let row = layers.row(Key::Direction);
        assert_eq!(
            (row.value.clone(), row.scope, row.overridden),
            (choice("ltr"), SettingScope::Medium, false)
        );
        assert_eq!(row.choices, choices(&["rtl", "ltr"]));
        assert_eq!(layers.row(Key::Fit).scope, SettingScope::Device);

        assert!(store.set_series_guess(work, "mode", "strip", 1).unwrap());
        store.set_series_setting(work, "direction", Some("rtl"), true, 2).unwrap();
        store.set_series_setting(work, "crop", Some("on"), true, 2).unwrap();
        let layers = Layers::read(&store, &config, Medium::Manga, Some(work), screen);
        let mode = layers.row(Key::Mode);
        assert!(mode.detected && !mode.overridden);
        assert_eq!(layers.word(Key::Mode), "strip");
        let direction = layers.row(Key::Direction);
        assert_eq!((direction.scope, direction.overridden), (SettingScope::Series, true));
        assert_eq!(layers.row(Key::CropBorders).scope, SettingScope::SeriesHere);
        assert!(layers.on(Key::CropBorders));
        // A guess never replaces a choice.
        assert!(!store.set_series_guess(work, "direction", "ltr", 3).unwrap());
    }

    /// A source's left to right or right to left is a paged book: it sets the mode as well as
    /// the direction, below the person's own choice and above the reader's guess, unless the
    /// catalogue counts the series a webtoon, which opens as the Webtoons default.
    #[test]
    fn a_direction_hint_means_a_paged_book() {
        let store = Store::open_in_memory().unwrap();
        let config = ReadingConfig::default();
        let work = "a:900403";
        let screen = Screen::default();
        let read = |hint, webtoon| {
            Layers::read(&store, &config, Medium::Manga, Some(work), screen)
                .with_source(SourceWord { hint, webtoon })
        };
        for (hint, direction) in
            [(ReadingHint::LeftToRight, "ltr"), (ReadingHint::RightToLeft, "rtl")]
        {
            let layers = read(Some(hint), false);
            let mode = layers.row(Key::Mode);
            assert_eq!(mode.value, choice("paged"), "{hint:?}");
            assert!(mode.detected && !mode.overridden, "{mode:?}");
            assert!(layers.sourced(Key::Mode) && layers.sourced(Key::Direction));
            assert_eq!(layers.word(Key::Direction), direction);
            let webtoon = read(Some(hint), true);
            assert_eq!(webtoon.word(Key::Mode), "strip", "a webtoon: the Webtoons default");
            assert_eq!(webtoon.word(Key::Direction), direction);
        }
        assert_eq!(read(None, true).word(Key::Mode), "strip", "the Webtoons default");

        // The reader's kept guess of a strip gives way to the hint.
        assert!(store.set_series_guess(work, "mode", "strip", 1).unwrap());
        assert_eq!(read(Some(ReadingHint::LeftToRight), false).word(Key::Mode), "paged");
        assert_eq!(read(None, false).word(Key::Mode), "strip");

        // The person's own mode for the series wins over the hint.
        store.set_series_setting(work, "mode", Some("vertical"), true, 2).unwrap();
        let layers = read(Some(ReadingHint::LeftToRight), false);
        assert_eq!(layers.word(Key::Mode), "vertical");
        assert!(!layers.sourced(Key::Mode) && layers.row(Key::Mode).overridden);
        assert_eq!(layers.word(Key::Direction), "ltr");
    }

    #[test]
    fn mode_and_direction_and_the_look_each_follow_by_their_own_switch() {
        let dir = tempfile::tempdir().unwrap();
        let paths = kaeri_core::config::Paths {
            config_file: dir.path().join("config.toml"),
            data_dir: dir.path().join("data"),
            cache_dir: dir.path().join("cache"),
        };
        let mut reading = ReadingConfig { look_follows: Some(false), ..Default::default() };
        let mut config = crate::config::ConfigState::new(
            kaeri_core::config::Config { reading: reading.clone(), ..Default::default() },
            None,
            &paths,
        );
        let store = Store::open_in_memory().unwrap();
        let work = "a:900402";
        let put = |config: &mut crate::config::ConfigState, key, name: &str| {
            set(
                config,
                &paths,
                &store,
                Medium::Manga,
                Some(work),
                key,
                SettingScope::Series,
                &choice(name),
                1,
            )
            .unwrap();
        };
        put(&mut config, Key::Mode, "strip");
        put(&mut config, Key::Colour, "greys");
        let scope = |setting: &str| {
            store
                .series_settings(work)
                .unwrap()
                .into_iter()
                .find(|s| s.setting == setting)
                .map(|s| s.scope)
        };
        assert_eq!(scope("mode").as_deref(), Some("series"), "mode follows");
        assert_eq!(scope("colour").as_deref(), Some("here"), "the look stays on the device");

        // The other way round, from an earlier version's one switch turned off.
        reading.look_follows = None;
        reading.series_choices_follow = false;
        reading.mode_follows = None;
        assert!(!follows(&reading, Key::Direction) && !follows(&reading, Key::Colour));
        reading.look_follows = Some(true);
        assert!(!follows(&reading, Key::Mode) && follows(&reading, Key::Colour));
    }

    #[test]
    fn rows_follow_the_medium_and_the_screen() {
        let store = Store::open_in_memory().unwrap();
        let config = ReadingConfig::default();
        let keys = |medium, screen| -> Vec<Key> {
            Layers::read(&store, &config, medium, None, screen)
                .rows()
                .iter()
                .map(|r| r.key)
                .collect()
        };
        let phone = Screen { handheld: true, ..Screen::default() };
        let manga = keys(Medium::Manga, phone);
        assert!(manga.contains(&Key::Direction) && manga.contains(&Key::PaperMode));
        assert!(!manga.contains(&Key::Font) && !manga.contains(&Key::RefreshText));
        assert!(!manga.contains(&Key::StripWidth));
        let novel = keys(Medium::Novel, phone);
        assert!(novel.contains(&Key::Font) && !novel.contains(&Key::Direction));
        let kaleido = Screen {
            eink: Some(EinkPanel::Kaleido3),
            refresh_hook: true,
            handheld: true,
            phone: true,
        };
        let rows = keys(Medium::Manga, kaleido);
        assert!(rows.contains(&Key::RainbowFilter) && rows.contains(&Key::RefreshText));
        assert!(!rows.contains(&Key::PaperMode) && !rows.contains(&Key::GallerySpeed));
        let layers = Layers::read(&store, &config, Medium::Manga, None, kaleido);
        assert_eq!(layers.amount(Key::RefreshList), 3.0);
        assert_eq!(layers.word(Key::Look), "paper");
        let grey = Screen { eink: Some(EinkPanel::Grey), ..kaleido };
        assert!(!keys(Medium::Manga, grey).contains(&Key::Colour));
    }

    #[test]
    fn tap_zones_start_where_the_board_says_and_show_where_they_apply() {
        let store = Store::open_in_memory().unwrap();
        let config = ReadingConfig::default();
        let zones = |medium, screen| Layers::read(&store, &config, medium, None, screen);
        let phone = Screen { handheld: true, phone: true, ..Screen::default() };
        let desktop = Screen::default();
        let eink = Screen { eink: Some(EinkPanel::Grey), ..phone };

        let on_phone = zones(Medium::Manga, phone).tap_zones();
        assert_eq!((on_phone.paged, on_phone.scrolling), (ZoneLayout::Auto, ZoneLayout::Off));
        assert_eq!(on_phone.size, ZoneSize::Standard);
        assert!(!on_phone.flip_sides && !on_phone.flip_ends && !on_phone.eink);
        assert!(
            on_phone.guide_seen_paged.is_empty() && on_phone.guide_seen_scrolling.is_empty()
        );
        assert_eq!(zones(Medium::Novel, desktop).tap_zones().paged, ZoneLayout::Sides);
        let on_eink = zones(Medium::Manga, eink).tap_zones();
        assert_eq!(on_eink.paged, ZoneLayout::Sides);
        assert!(on_eink.eink && on_eink.swipes_turn);

        let keys = |screen| -> Vec<Key> {
            zones(Medium::Manga, screen).rows().iter().map(|r| r.key).collect()
        };
        let rows = keys(phone);
        for key in
            [Key::TapPaged, Key::TapScroll, Key::TapSize, Key::TapFlipSides, Key::TapFlipEnds]
        {
            assert!(rows.contains(&key), "{key:?}");
            assert_eq!(table(key, Medium::Novel), TAPS, "one value whatever the medium");
        }
        assert!(!rows.contains(&Key::SwipeTurns) && !rows.contains(&Key::TapLayout));
        let rows = keys(eink);
        assert!(rows.contains(&Key::SwipeTurns) && rows.contains(&Key::TapPaged));
        assert!(!rows.contains(&Key::TapScroll), "e-ink has one choice for all reading");
        let row = zones(Medium::Manga, phone).row(Key::TapPaged);
        assert_eq!(row.scope, SettingScope::Device);
        assert_eq!(row.choices.len(), 7);
        assert_eq!(zones(Medium::Manga, phone).row(Key::TapScroll).choices.len(), 6);
    }

    /// The setting before tap zones was one per medium, Edges, Halves (kept as `sides`) or
    /// Off. Unset, the paged layout reads it: Off stays Off and the rest is Sides.
    #[test]
    fn the_earlier_tap_setting_carries_over_as_sides_or_off() {
        let store = Store::open_in_memory().unwrap();
        let phone = Screen { handheld: true, phone: true, ..Screen::default() };
        let read = |config: &ReadingConfig, medium| {
            Layers::read(&store, config, medium, None, phone).tap_zones()
        };
        let text = |s: &str| toml::Value::String(s.to_owned());
        for (earlier, now) in [
            ("edges", ZoneLayout::Sides),
            ("sides", ZoneLayout::Sides),
            ("off", ZoneLayout::Off),
        ] {
            let mut config = ReadingConfig::default();
            config.manga.insert("tap_layout".into(), text(earlier));
            assert_eq!(read(&config, Medium::Manga).paged, now, "{earlier}");
            assert_eq!(read(&config, Medium::Novel).paged, now, "the other medium's, unset");
            assert_eq!(read(&config, Medium::Manga).scrolling, ZoneLayout::Off);
            let row =
                Layers::read(&store, &config, Medium::Manga, None, phone).row(Key::TapPaged);
            assert_eq!(row.value, choice(now.name()), "the row says the new name");
        }
        // Each medium keeps its own until the person chooses.
        let mut config = ReadingConfig::default();
        config.manga.insert("tap_layout".into(), text("edges"));
        config.novel.insert("tap_layout".into(), text("off"));
        assert_eq!(read(&config, Medium::Manga).paged, ZoneLayout::Sides);
        assert_eq!(read(&config, Medium::Novel).paged, ZoneLayout::Off);
        // A choice of the new setting wins, and is the same for both media.
        config.taps.insert("tap_paged".into(), text("l_shape"));
        assert_eq!(read(&config, Medium::Manga).paged, ZoneLayout::LShape);
        assert_eq!(read(&config, Medium::Novel).paged, ZoneLayout::LShape);
        // A hand-written old name reads as Sides, and an unknown one as the default.
        config.taps.insert("tap_paged".into(), text("halves"));
        assert_eq!(read(&config, Medium::Manga).paged, ZoneLayout::Sides);
        config.taps.insert("tap_paged".into(), text("diagonal"));
        assert_eq!(read(&config, Medium::Manga).paged, ZoneLayout::Sides, "the earlier value");
        let mut config = ReadingConfig::default();
        config.taps.insert("tap_paged".into(), text("diagonal"));
        config.taps.insert("tap_scroll".into(), text("auto"));
        config.taps.insert("tap_size".into(), text("tiny"));
        let zones = read(&config, Medium::Manga);
        assert_eq!((zones.paged, zones.scrolling), (ZoneLayout::Auto, ZoneLayout::Off));
        assert_eq!(zones.size, ZoneSize::Standard);
    }

    #[test]
    fn tap_settings_and_seen_guides_are_kept_in_the_devices_taps_table() {
        let dir = tempfile::tempdir().unwrap();
        let paths = kaeri_core::config::Paths {
            config_file: dir.path().join("config.toml"),
            data_dir: dir.path().join("data"),
            cache_dir: dir.path().join("cache"),
        };
        let mut config = crate::config::ConfigState::new(
            kaeri_core::config::Config::default(),
            None,
            &paths,
        );
        let store = Store::open_in_memory().unwrap();
        let put = |config: &mut crate::config::ConfigState, key: Key, value: ReaderValue| {
            let scope = key.home_scope();
            set(config, &paths, &store, Medium::Novel, None, key, scope, &value, 1).unwrap();
        };
        put(&mut config, Key::TapScroll, choice("top_bottom"));
        put(&mut config, Key::TapSize, choice("small"));
        put(&mut config, Key::TapFlipSides, ReaderValue::Toggle { on: true });
        guide_seen(&mut config, &paths, "paged:sides:small:sides:rtl").unwrap();
        guide_seen(&mut config, &paths, "nothing").unwrap();

        let reading = &config.effective().reading;
        assert!(reading.novel.is_empty() && reading.manga.is_empty());
        let zones =
            Layers::read(&store, reading, Medium::Manga, None, Screen::default()).tap_zones();
        assert_eq!(zones.scrolling, ZoneLayout::TopBottom);
        assert_eq!(zones.size, ZoneSize::Small);
        assert!(zones.flip_sides && !zones.flip_ends);
        assert_eq!(zones.guide_seen_paged, "paged:sides:small:sides:rtl");
        assert!(zones.guide_seen_scrolling.is_empty());
        let text = std::fs::read_to_string(&paths.config_file).unwrap();
        assert!(text.contains("[reading.taps]"), "{text}");
    }

    #[test]
    fn novel_text_is_justified_unless_the_screen_is_a_phone() {
        let store = Store::open_in_memory().unwrap();
        let config = ReadingConfig::default();
        let justified = |medium, screen| {
            Layers::read(&store, &config, medium, None, screen).on(Key::Justify)
        };
        let phone = Screen { handheld: true, phone: true, ..Screen::default() };
        let tablet = Screen { handheld: true, ..Screen::default() };
        let eink_phone = Screen { eink: Some(EinkPanel::Grey), ..phone };
        assert!(!justified(Medium::Novel, phone));
        assert!(justified(Medium::Novel, tablet));
        assert!(justified(Medium::Novel, Screen::default()));
        assert!(justified(Medium::Novel, eink_phone));
        assert!(!justified(Medium::Manga, tablet));
    }
}
