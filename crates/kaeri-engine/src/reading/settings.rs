//! The reader's settings and where each value comes from.
//!
//! A value is looked for in layers: the series' own choice (synced, this device only, or the
//! reader's guess), then the medium's value in `config.toml` (`[reading.manga]`,
//! `[reading.novel]`; the choices among them sync as shared keys), then the default. The e-ink
//! refresh settings are the device's, in `[reading.eink]`, whatever the medium.

use std::collections::BTreeMap;

use kaeri_core::{config::ReadingConfig, media::Medium, settings::SettingValue};
use kaeri_model::{
    eink::{EinkMatch, EinkPanel, EinkProfile},
    reader::{ReaderSettingKey as Key, ReaderSettingRow, ReaderValue, SettingScope},
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
        Key::Mode => choices(&["paged", "strip"]),
        Key::Direction => choices(&["rtl", "ltr"]),
        Key::Colour => choices(&["auto", "colour", "greys"]),
        Key::Look => choices(&["paper", "light", "sepia", "dark"]),
        Key::Font => choices(&["serif", "sans"]),
        Key::Fit => choices(&["screen", "width"]),
        Key::PageLayout => choices(&["auto", "single", "double"]),
        Key::Spreads => choices(&["split", "rotate", "whole"]),
        Key::TapLayout => choices(&["edges", "sides", "off"]),
        Key::GallerySpeed => choices(&["standard", "fast", "best"]),
        Key::FootValue => choices(&["pages_left", "none"]),
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
        Key::PageLayout => choice("auto"),
        Key::Spreads => choice("split"),
        Key::TextSize | Key::Margins | Key::Contrast => amount(1.0),
        Key::LineSpacing => amount(1.4),
        Key::Justify => on(medium == Medium::Novel && (screen.eink.is_some() || !screen.phone)),
        Key::TapLayout => choice("edges"),
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
        Key::Folio => on(true),
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
        Key::PaperMode | Key::GreyscalePages | Key::QuietPhone => {
            screen.handheld && screen.eink.is_none()
        }
        Key::Colour => screen.eink != Some(EinkPanel::Grey),
        Key::StripWidth => !screen.handheld,
        Key::VolumeKeysTurn => screen.handheld || screen.eink.is_some(),
        _ => true,
    }
}

/// The config table a setting's medium value is kept in.
pub fn table(key: Key, medium: Medium) -> [&'static str; 2] {
    match key {
        Key::RefreshImage | Key::RefreshText | Key::RefreshList | Key::GallerySpeed => {
            ["reading", "eink"]
        }
        _ if medium == Medium::Novel => ["reading", "novel"],
        _ => ["reading", "manga"],
    }
}

fn map(config: &ReadingConfig, key: Key, medium: Medium) -> &BTreeMap<String, toml::Value> {
    match table(key, medium)[1] {
        "eink" => &config.eink,
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

/// Everything the layers say, read once for a sheet or a session.
#[derive(Debug, Clone)]
pub struct Layers {
    pub medium: Medium,
    pub screen: Screen,
    series: Vec<SeriesSetting>,
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
        Self { medium, screen, series, config: config.clone() }
    }

    /// One setting's row: its value and where the value comes from.
    pub fn row(&self, key: Key) -> ReaderSettingRow {
        let series = key
            .series_scope()
            .and_then(|_| self.series.iter().find(|s| s.setting == key.name()));
        let medium = map(&self.config, key, self.medium).get(key.name()).and_then(from_toml);
        let (value, scope, detected, overridden) = match (series, medium) {
            (Some(s), _) if s.scope == "auto" => {
                (ReaderValue::decode(&s.value), SettingScope::Series, true, false)
            }
            (Some(s), _) => {
                let scope = if s.scope == "series" {
                    SettingScope::Series
                } else {
                    SettingScope::SeriesHere
                };
                (ReaderValue::decode(&s.value), scope, false, true)
            }
            (None, Some(value)) => (value, key.home_scope(), false, false),
            (None, None) => {
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
