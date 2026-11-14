//! The reader settings overlay, `,` in the reader: the settings a terminal honours, each under
//! the place a change is kept, changed in place with the arrows and applied at once.
//!
//! A light novel has its mode, the text width, the line spacing and justify. Manga has its
//! mode and direction, fit, page layout, spreads, crop borders, the strip's width and skipping
//! credit pages. Font, size, look, colour and tap zones follow the terminal or need a touch
//! screen; the overlay says so in a note rather than offering rows that would do nothing.

use kaeri_core::media::Medium;
use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    reader::{
        ReaderCommand, ReaderSettingKey as Key, ReaderSettingRow, ReaderValue, SettingScope,
    },
};

use super::{
    App, OPEN_AMOUNT, OPEN_SPACING, TEXT_MARGIN, TEXT_WIDTH, TEXT_WIDTH_MIN, TEXT_WIDTH_STEP,
    TIGHT_AMOUNT, strip_cols,
};
use crate::{keymap::Action, nav::Overlay};

/// The settings a light novel offers in the terminal, in the board's order.
const NOVEL_KEYS: &[Key] = &[Key::Mode, Key::TextWidth, Key::LineSpacing, Key::Justify];

/// The settings manga offers in the terminal.
const MANGA_KEYS: &[Key] = &[
    Key::Mode,
    Key::Direction,
    Key::Fit,
    Key::PageLayout,
    Key::Spreads,
    Key::CropBorders,
    Key::StripWidth,
    Key::SkipCreditPages,
];

/// The strip's width moves a tenth of the window at a time, from three tenths to all of it.
const STRIP_STEP: f64 = 0.1;
const STRIP_LEAST: f64 = 0.3;

/// Where a change to a row is kept, which is the heading the row sits under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SettingPlace {
    /// This series' own choice, synced: mode and direction, as `m` keeps them.
    Series,
    /// The medium on this computer.
    Here,
    /// The medium everywhere, synced.
    All,
}

impl SettingPlace {
    fn scope(self) -> SettingScope {
        match self {
            Self::Series => SettingScope::Series,
            Self::Here => SettingScope::Device,
            Self::All => SettingScope::Medium,
        }
    }

    /// The heading over the rows kept here.
    pub fn heading(self, medium: Medium) -> String {
        let novel = medium == Medium::Novel;
        match self {
            Self::Series => t::tui_reader_settings_series(),
            Self::Here if novel => t::tui_reader_settings_novels_here(),
            Self::Here => t::tui_reader_settings_manga_here(),
            Self::All if novel => t::tui_reader_settings_novels_all(),
            Self::All => t::tui_reader_settings_manga_all(),
        }
    }
}

/// How a row's value reads.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingShown {
    /// Every choice, the one in effect marked.
    Choices(Vec<(String, bool)>),
    /// An amount between arrows: "‹ 50 columns ›".
    Amount(String),
    /// On or Off.
    Toggle(bool),
}

/// One row of the overlay.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingItem {
    pub key: Key,
    pub place: SettingPlace,
    pub label: String,
    pub shown: SettingShown,
    /// A series' own choice: `d` drops it for the default.
    pub overridden: bool,
}

/// A row's label.
fn label(key: Key) -> String {
    match key {
        Key::Mode => t::tui_reader_setting_mode(),
        Key::Direction => t::tui_reader_setting_direction(),
        Key::Fit => t::tui_reader_setting_fit(),
        Key::PageLayout => t::tui_reader_setting_page_layout(),
        Key::Spreads => t::tui_reader_setting_spreads(),
        Key::CropBorders => t::tui_reader_setting_crop(),
        Key::StripWidth => t::tui_reader_setting_strip_width(),
        Key::SkipCreditPages => t::settings_skip_credits(),
        Key::TextWidth => t::tui_reader_setting_text_width(),
        Key::LineSpacing => t::tui_reader_setting_line_spacing(),
        Key::Justify => t::tui_reader_setting_justify(),
        _ => String::new(),
    }
}

/// A choice's words.
fn choice_words(key: Key, name: &str) -> String {
    // The engine's names for the choices.
    match (key, name) {
        (Key::Mode, "paged") => t::tui_reader_value_paged(),
        (Key::Mode, "scrolled") => t::tui_reader_value_scrolled(),
        (Key::Mode, "vertical") => t::tui_reader_value_vertical(),
        (Key::Mode, "strip") => t::reader_mode_strip(),
        (Key::Direction, "rtl") => t::reader_mode_rtl(),
        (Key::Direction, "ltr") => t::reader_mode_ltr(),
        (Key::Fit, "screen") => t::reader_fit_screen(),
        (Key::Fit, "width") => t::reader_fit_width(),
        (Key::PageLayout, "auto") => t::reader_layout_auto(),
        (Key::PageLayout, "single") => t::reader_layout_single(),
        (Key::PageLayout, "double") => t::reader_layout_double(),
        (Key::Spreads, "split") => t::reader_spreads_split(),
        (Key::Spreads, "rotate") => t::reader_spreads_rotate(),
        (Key::Spreads, "whole") => t::reader_spreads_whole(),
        _ => name.to_owned(),
    }
}

/// The choices a row offers in the terminal: the engine's, less the fits a terminal can't
/// show (height), else the ones the terminal knows.
fn choice_names(row: &ReaderSettingRow, medium: Medium) -> Vec<String> {
    let mut names: Vec<String> = row
        .choices
        .iter()
        .filter_map(|c| match c {
            ReaderValue::Choice { name } => Some(name.clone()),
            _ => None,
        })
        .collect();
    if names.is_empty() {
        // The engine's names for the choices.
        let known: &[&str] = match (row.key, medium) {
            (Key::Mode, Medium::Novel) => &["paged", "scrolled"], // i18n-skip: values
            (Key::Mode, _) => &["paged", "strip"],                // i18n-skip: values
            (Key::Direction, _) => &["rtl", "ltr"],               // i18n-skip: values
            (Key::Fit, _) => &["screen", "width"],                // i18n-skip: values
            (Key::PageLayout, _) => &["auto", "single", "double"], // i18n-skip: values
            (Key::Spreads, _) => &["split", "rotate", "whole"],   // i18n-skip: values
            _ => &[],
        };
        names = known.iter().map(|n| (*n).to_owned()).collect();
    }
    if row.key == Key::Fit {
        names.retain(|n| n == "screen" || n == "width"); // i18n-skip: values
    }
    names
}

impl App {
    /// The overlay's rows for the open chapter, grouped by where a change is kept.
    pub fn reader_setting_items(&self) -> Vec<SettingItem> {
        let Some(reader) = self.reader_here() else { return Vec::new() };
        let medium = reader.medium().unwrap_or(Medium::Manga);
        let Some(rows) =
            reader.settings.as_ref().or(reader.view.as_ref().and_then(|v| v.settings.as_ref()))
        else {
            return Vec::new();
        };
        let keys = if medium == Medium::Novel { NOVEL_KEYS } else { MANGA_KEYS };
        let width = self.window.map_or(80, |(w, _)| w);
        let mut items: Vec<SettingItem> = keys
            .iter()
            .filter_map(|key| {
                let row = rows.rows.iter().find(|r| r.key == *key)?;
                let place = match key.series_scope() {
                    Some(SettingScope::Series) => SettingPlace::Series,
                    _ if key.syncs() => SettingPlace::All,
                    _ => SettingPlace::Here,
                };
                let shown = match (key, &row.value) {
                    (Key::LineSpacing, ReaderValue::Amount { value }) => {
                        let open = *value >= OPEN_SPACING;
                        SettingShown::Choices(vec![
                            (t::tui_reader_value_open(), open),
                            (t::tui_reader_value_tight(), !open),
                        ])
                    }
                    (Key::TextWidth, ReaderValue::Amount { value }) => {
                        SettingShown::Amount(t::tui_reader_value_columns(value.round() as i64))
                    }
                    (Key::StripWidth, ReaderValue::Amount { .. }) => {
                        let cols = strip_cols(reader, width);
                        SettingShown::Amount(t::tui_reader_value_columns(i64::from(cols)))
                    }
                    (_, ReaderValue::Choice { name }) => SettingShown::Choices(
                        choice_names(row, medium)
                            .into_iter()
                            .map(|n| (choice_words(*key, &n), n == *name))
                            .collect(),
                    ),
                    (_, ReaderValue::Toggle { on }) => SettingShown::Toggle(*on),
                    _ => return None,
                };
                Some(SettingItem {
                    key: *key,
                    place,
                    label: label(*key),
                    shown,
                    overridden: row.overridden,
                })
            })
            .collect();
        items.sort_by_key(|i| i.place);
        items
    }

    /// `,`: the reader settings, open at once on the settings the chapter came with, and
    /// asked for again so a change made elsewhere shows.
    pub(super) fn open_reader_settings(&mut self) -> Option<Command> {
        let reader = self.reader_mut()?;
        reader.settings_asked = true;
        let command = Command::Reader(ReaderCommand::LoadSettings {
            medium: reader.medium().unwrap_or(Medium::Manga),
            work: Some(reader.work.clone()),
        });
        self.cursors.overlay.selected = 0;
        self.nav.open_overlay(Overlay::ReaderSettings);
        Some(command)
    }

    /// A key in the settings overlay: `j` and `k` move, the arrows and Enter change the row,
    /// `d` drops a series' own choice.
    pub(super) fn reader_settings_key(&mut self, action: Action) -> Option<Command> {
        let items = self.reader_setting_items();
        let len = items.len();
        let at = self.cursors.overlay.selected.min(len.saturating_sub(1));
        let moved = match action {
            Action::Down => (at + 1).min(len.saturating_sub(1)),
            Action::Up => at.saturating_sub(1),
            Action::Top => 0,
            Action::Bottom => len.saturating_sub(1),
            Action::Right | Action::Open => return self.change_setting(items.get(at)?, true),
            Action::Left => return self.change_setting(items.get(at)?, false),
            Action::Download => {
                let item = items.get(at).filter(|i| i.overridden)?;
                let reader = self.reader_here()?;
                return Some(Command::Reader(ReaderCommand::UseDefault {
                    medium: reader.medium().unwrap_or(Medium::Manga),
                    work: reader.work.clone(),
                    key: item.key,
                }));
            }
            _ => at,
        };
        self.cursors.overlay.selected = moved;
        None
    }

    /// The next value of a row, or the one before, set at once.
    fn change_setting(&mut self, item: &SettingItem, on: bool) -> Option<Command> {
        let reader = self.reader_here()?;
        let value = match (&item.shown, item.key) {
            (SettingShown::Choices(_), Key::LineSpacing) => {
                let open = reader.amount(Key::LineSpacing).is_some_and(|a| a >= OPEN_SPACING);
                ReaderValue::Amount { value: if open { TIGHT_AMOUNT } else { OPEN_AMOUNT } }
            }
            (SettingShown::Choices(choices), key) => {
                let medium = reader.medium().unwrap_or(Medium::Manga);
                let settings = reader
                    .settings
                    .as_ref()
                    .or(reader.view.as_ref().and_then(|v| v.settings.as_ref()))?;
                let row = settings.rows.iter().find(|r| r.key == key)?;
                let names = choice_names(row, medium);
                let now = choices.iter().position(|(_, current)| *current).unwrap_or(0);
                let next = if on {
                    (now + 1) % names.len()
                } else {
                    (now + names.len() - 1) % names.len()
                };
                ReaderValue::Choice { name: names.get(next)?.clone() }
            }
            (SettingShown::Toggle(now), _) => ReaderValue::Toggle { on: !now },
            (SettingShown::Amount(_), Key::TextWidth) => {
                let (w, _) = self.window?;
                let most = w.saturating_sub(2 * TEXT_MARGIN).max(TEXT_WIDTH_MIN);
                let now = reader
                    .amount(Key::TextWidth)
                    .map_or(TEXT_WIDTH, |v| v.round().clamp(1.0, f64::from(u16::MAX)) as u16)
                    .clamp(TEXT_WIDTH_MIN, most);
                let width = if on {
                    now.saturating_add(TEXT_WIDTH_STEP).min(most)
                } else {
                    now.saturating_sub(TEXT_WIDTH_STEP).max(TEXT_WIDTH_MIN)
                };
                ReaderValue::Amount { value: f64::from(width) }
            }
            (SettingShown::Amount(_), Key::StripWidth) => {
                let now = reader.amount(Key::StripWidth).unwrap_or(0.6);
                let step = if on { STRIP_STEP } else { -STRIP_STEP };
                let value = ((now + step) * 10.0).round() / 10.0;
                ReaderValue::Amount { value: value.clamp(STRIP_LEAST, 1.0) }
            }
            (SettingShown::Amount(_), _) => return None,
        };
        let same = match (
            &value,
            reader.settings.as_ref().and_then(|s| s.rows.iter().find(|r| r.key == item.key)),
        ) {
            (value, Some(row)) => row.value == *value,
            _ => false,
        };
        if same {
            return None;
        }
        self.set_reader_setting(item.key, item.place.scope(), value)
    }

    /// Whether the open chapter is a novel, for the overlay's words.
    pub fn settings_medium(&self) -> Medium {
        self.reader_here().and_then(super::Reader::medium).unwrap_or(Medium::Manga)
    }
}
