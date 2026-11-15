//! The reader settings overlay, `,` in the reader: the settings a terminal honours, each under
//! the place a change is kept, changed in place with the arrows and applied at once.
//!
//! A light novel has its mode, the text width, the line spacing and justify, all under the
//! novels on this computer as the board groups them. Manga has its mode and direction, fit,
//! page layout, spreads, crop borders, skipping credit pages and the strip's gap, the board's
//! rows under the engine's scopes. Font, size, look, colour and tap zones follow the terminal
//! or need a touch screen; the overlay says so in a note rather than offering rows that would
//! do nothing.

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
    TIGHT_AMOUNT,
};
use crate::{keymap::Action, nav::Overlay};

/// The settings a light novel offers in the terminal, in the board's order.
const NOVEL_KEYS: &[Key] = &[Key::Mode, Key::TextWidth, Key::LineSpacing, Key::Justify];

/// The settings manga offers in the terminal, in the board's order.
const MANGA_KEYS: &[Key] = &[
    Key::Mode,
    Key::Direction,
    Key::Fit,
    Key::PageLayout,
    Key::Spreads,
    Key::CropBorders,
    Key::SkipCreditPages,
    Key::StripGap,
];

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
    /// Where a change is sent to be kept, which is the place's own except for a novel's mode.
    pub scope: SettingScope,
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
        Key::StripGap => t::tui_reader_setting_strip_gap(),
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
        (Key::Mode, "paged") => t::tui_reader_value_paged(), // i18n-skip: a value
        (Key::Mode, "scrolled") => t::tui_reader_value_scrolled(), // i18n-skip: a value
        (Key::Mode, "vertical") => t::tui_reader_value_vertical(), // i18n-skip: a value
        (Key::Mode, "strip") => t::reader_mode_strip(),      // i18n-skip: a value
        (Key::Direction, "rtl") => t::reader_mode_rtl(),     // i18n-skip: a value
        (Key::Direction, "ltr") => t::reader_mode_ltr(),     // i18n-skip: a value
        (Key::Fit, "screen") => t::reader_fit_screen(),      // i18n-skip: a value
        (Key::Fit, "width") => t::reader_fit_width(),        // i18n-skip: a value
        (Key::Fit, "height") => t::tui_reader_value_height(), // i18n-skip: a value
        (Key::PageLayout, "auto") => t::reader_layout_auto(), // i18n-skip: a value
        (Key::PageLayout, "single") => t::reader_layout_single(), // i18n-skip: a value
        (Key::PageLayout, "double") => t::reader_layout_double(), // i18n-skip: a value
        (Key::Spreads, "split") => t::reader_spreads_split(), // i18n-skip: a value
        (Key::Spreads, "rotate") => t::reader_spreads_rotate(), // i18n-skip: a value
        (Key::Spreads, "whole") => t::reader_spreads_whole(), // i18n-skip: a value
        (Key::StripGap, "none") => t::reader_gap_none(),     // i18n-skip: a value
        (Key::StripGap, "small") => t::reader_gap_small(),   // i18n-skip: a value
        _ => name.to_owned(),
    }
}

/// The choices a row offers in the terminal: the engine's, else the ones the terminal knows.
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
            (Key::Fit, _) => &["screen", "width", "height"],      // i18n-skip: values
            (Key::PageLayout, _) => &["auto", "single", "double"], // i18n-skip: values
            (Key::Spreads, _) => &["split", "rotate", "whole"],   // i18n-skip: values
            (Key::StripGap, _) => &["none", "small"],             // i18n-skip: values
            _ => &[],
        };
        names = known.iter().map(|n| (*n).to_owned()).collect();
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
        let mut items: Vec<SettingItem> = keys
            .iter()
            .filter_map(|key| {
                let row = rows.rows.iter().find(|r| r.key == *key)?;
                let (scope, place) = match key.series_scope() {
                    Some(SettingScope::Series) => (SettingScope::Series, SettingPlace::Series),
                    _ if key.syncs() => (SettingScope::Medium, SettingPlace::All),
                    _ => (SettingScope::Device, SettingPlace::Here),
                };
                // The board puts a novel's four rows together under the novels on this
                // computer. Mode is still kept for the series, as `m` keeps it.
                let place = if medium == Medium::Novel { SettingPlace::Here } else { place };
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
                    scope,
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
                let open = reader.amount(Key::LineSpacing).is_none_or(|a| a >= OPEN_SPACING);
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
                if names.is_empty() {
                    return None;
                }
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
        match item.key {
            // A new fit starts the page at its reading side, as `f` does.
            Key::Fit => {
                let reader = self.reader_mut()?;
                reader.zoom = None;
                reader.scroll = 0;
            }
            // The strip keeps the image at the top, a gap or none between them.
            Key::StripGap => {
                let place = self.strip_place();
                let task = self.set_reader_setting(item.key, item.scope, value);
                self.keep_strip_place_at(place);
                return task;
            }
            _ => {}
        }
        self.set_reader_setting(item.key, item.scope, value)
    }

    /// Whether the open chapter is a novel, for the overlay's words.
    pub fn settings_medium(&self) -> Medium {
        self.reader_here().and_then(super::Reader::medium).unwrap_or(Medium::Manga)
    }
}
