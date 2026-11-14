//! The help overlay's content. It is generated from the live tables, never written by hand,
//! so a rebind shows up in it and the documentation cannot drift from what the keys do.

use std::collections::BTreeMap;

use super::{Action, Binding, Keymap, Scope, said};

impl Keymap {
    /// The help overlay's content, grouped by scope.
    ///
    /// Generated from the live table — this *is* the documentation, so it cannot drift.
    pub fn help(&self) -> Vec<(Scope, Vec<(String, &'static str)>)> {
        // The jump keys are one concept, collapsed to a range rather than listed individually —
        // and the range is read off the live table rather than written down, because a hardcoded
        // one silently went stale when a ninth section was added.
        let jump_range = {
            let mut numbers: Vec<u8> = self
                .bindings
                .values()
                .filter_map(|action| match action {
                    Action::JumpSection(n) => Some(*n),
                    _ => None,
                })
                .collect();
            numbers.sort_unstable();
            // Keys, not numbers: the tenth section is on `0`.
            let key = |n: &u8| n % 10;
            match (numbers.first(), numbers.last()) {
                (Some(first), Some(last)) if first != last => {
                    format!("{}–{}", key(first), key(last)) // i18n-skip
                }
                (Some(only), _) => key(only).to_string(),
                _ => String::new(),
            }
        };

        let mut grouped: BTreeMap<Scope, BTreeMap<Action, Vec<Binding>>> = BTreeMap::new();
        for (binding, action) in self.bindings.iter().chain(self.playback.iter()) {
            if matches!(action, Action::JumpSection(n) if *n != 1) {
                continue;
            }
            grouped
                .entry(action.scope())
                .or_default()
                .entry(*action)
                .or_default()
                .push(*binding);
        }

        Scope::ALL
            .into_iter()
            .filter_map(|scope| {
                let actions = grouped.remove(&scope)?;
                let rows = actions
                    .into_iter()
                    .map(|(action, mut keys)| {
                        keys.sort();
                        if matches!(action, Action::JumpSection(_)) {
                            // One row for all of them, so it is labelled for all of them.
                            return (
                                jump_range.clone(),
                                said(kaeri_i18n::t::keys_help_jump_section()),
                            );
                        }
                        let rendered =
                            keys.iter().map(Binding::render).collect::<Vec<_>>().join(" / ");
                        (rendered, action.label())
                    })
                    .collect::<Vec<_>>();
                Some((scope, rows))
            })
            .collect()
    }
}

/// The reader's key reference, in the order its columns list them. Keys that only work in one
/// state, Try again and Skip, are left out: the status line names them where they work.
const READER_ROWS: &[Action] = &[
    Action::ReaderNext,
    Action::ReaderPrevious,
    Action::ReaderLeft,
    Action::ReaderRight,
    Action::ReaderScrollDown,
    Action::ReaderScrollUp,
    Action::ReaderFirst,
    Action::ReaderLast,
    Action::ReaderNextChapter,
    Action::ReaderPreviousChapter,
    Action::ReaderChapters,
    Action::ReaderBackToPage,
    Action::ReaderMarkRead,
    Action::ReaderDownload,
    Action::ReaderBare,
];

const READER_VIEW_ROWS: &[Action] = &[
    Action::ReaderMode,
    Action::ReaderFit,
    Action::ReaderZoom,
    Action::ReaderZoomIn,
    Action::ReaderZoomOut,
    Action::ReaderPages,
    Action::ReaderNarrower,
    Action::ReaderWider,
    Action::ReaderSettings,
];

const READER_GLOBAL_ROWS: &[Action] = &[
    Action::Help,
    Action::CommandPalette,
    Action::ReaderLeave,
    Action::Quit,
    Action::ShowLogs,
];

impl Keymap {
    /// The reader's key reference: its own keys first, then how it shows pages, then the global
    /// keys it keeps, each with the keys that reach it in the reader. Read off the live tables,
    /// so a rebind shows; an action with no key is left out.
    ///
    /// Keys that do nothing in the open chapter are left out: no fit, zoom or two pages in a
    /// light novel (`novel`), and no text width in manga.
    pub fn reader_help(&self, novel: bool) -> Vec<(&'static str, Vec<(String, &'static str)>)> {
        let here = |action: &Action| {
            if novel {
                !matches!(
                    action,
                    Action::ReaderFit
                        | Action::ReaderZoom
                        | Action::ReaderZoomIn
                        | Action::ReaderZoomOut
                        | Action::ReaderPages
                )
            } else {
                !matches!(action, Action::ReaderNarrower | Action::ReaderWider)
            }
        };
        let label = |action: Action| match action {
            Action::ReaderMode if novel => said(kaeri_i18n::t::tui_reader_action_mode_novel()),
            _ => action.label(),
        };
        let rows = |actions: &[Action]| -> Vec<(String, &'static str)> {
            actions
                .iter()
                .filter(|action| here(action))
                .filter_map(|action| {
                    let mut keys = self.keys_in(*action, super::Context::Reading);
                    if keys.is_empty() {
                        return None;
                    }
                    keys.sort();
                    // The arrow is the turn key; its letter is the vim alias. Scrolling is the
                    // other way round: j and k first, as the board lists them.
                    if matches!(action, Action::ReaderScrollDown | Action::ReaderScrollUp) {
                        keys.sort_by_key(|b| {
                            !matches!(b.code, crossterm::event::KeyCode::Char(_))
                        });
                    }
                    if matches!(action, Action::ReaderLeft | Action::ReaderRight) {
                        keys.sort_by_key(|b| {
                            !matches!(
                                b.code,
                                crossterm::event::KeyCode::Left
                                    | crossterm::event::KeyCode::Right
                            )
                        });
                    }
                    let rendered =
                        keys.iter().map(Binding::render).collect::<Vec<_>>().join(" / ");
                    Some((rendered, label(*action)))
                })
                .collect()
        };
        vec![
            (Scope::Reader.heading(), rows(READER_ROWS)),
            (Scope::ReaderView.heading(), rows(READER_VIEW_ROWS)),
            (Scope::Global.heading(), rows(READER_GLOBAL_ROWS)),
        ]
        .into_iter()
        .filter(|(_, rows)| !rows.is_empty())
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::Context;

    #[test]
    fn the_order_keys_have_a_providers_heading_and_palette_entries() {
        let keymap = Keymap::new();
        let providers = keymap
            .help()
            .into_iter()
            .find(|(s, _)| *s == Scope::Providers)
            .map(|(_, rows)| rows)
            .expect("a PROVIDERS group");
        assert_eq!(Scope::Providers.heading(), kaeri_i18n::t::keys_scope_sources());
        assert_eq!(
            providers,
            [
                ("K".to_string(), "Move source up"),
                ("J".to_string(), "Move source down"),
                ("B".to_string(), "Play the demo"),
                ("Y".to_string(), "Remove or add the demo"),
            ]
        );
        let entries = keymap.palette_entries("move source", Context::Browsing);
        assert!(entries.contains(&(Action::RaiseSource, "K".into())), "{entries:?}");
        assert!(entries.contains(&(Action::LowerSource, "J".into())), "{entries:?}");
        assert_eq!(Action::LowerSource.config_key(), "lower_source");
        assert_eq!(Action::RaiseSource.where_it_works(), kaeri_i18n::t::keys_where_sources());
    }

    #[test]
    fn help_is_generated_from_the_live_keymap_and_follows_a_rebind() {
        // The rule this module exists to enforce: documentation cannot drift from
        // behaviour, because it is derived from it.
        let mut keymap = Keymap::new();
        let before = keymap.help();
        let title_rows = |help: &Vec<(Scope, Vec<(String, &'static str)>)>| {
            help.iter()
                .find(|(s, _)| *s == Scope::Title)
                .map(|(_, rows)| rows.clone())
                .unwrap_or_default()
        };
        assert!(title_rows(&before).iter().any(|(k, _)| k == "e"));

        keymap.apply_overrides(&BTreeMap::from([(
            "show_episodes".to_string(),
            "ctrl+e".to_string(),
        )]));
        let after = title_rows(&keymap.help());
        assert!(after.iter().any(|(k, l)| k == "ctrl+e" && *l == "Episodes"));
        assert!(
            !after.iter().any(|(k, l)| k == "e" && *l == "Episodes"),
            "stale binding still documented"
        );
    }

    #[test]
    fn help_collapses_the_eight_jump_keys_into_one_row() {
        // Eight near-identical rows would bury everything else in the global section.
        let keymap = Keymap::new();
        let global = keymap
            .help()
            .into_iter()
            .find(|(s, _)| *s == Scope::Global)
            .map(|(_, rows)| rows)
            .unwrap();
        let jump_rows: Vec<_> =
            global.iter().filter(|(_, label)| *label == "Go to section").collect();
        assert_eq!(jump_rows.len(), 1);
        // Read off the live table, so adding a section cannot leave the overlay claiming a
        // range that stops short of it. The tenth is on `0`: the range names keys.
        assert_eq!(crate::nav::Section::ALL.len(), 10);
        assert_eq!(jump_rows[0].0, "1–0");
    }

    #[test]
    fn help_covers_every_scope_that_has_bindings() {
        let help = Keymap::new().help();
        let scopes: Vec<Scope> = help.iter().map(|(s, _)| *s).collect();
        for expected in [Scope::Global, Scope::Movement, Scope::Lists, Scope::Title] {
            assert!(scopes.contains(&expected), "{expected:?} missing from help");
        }
        assert!(help.iter().all(|(_, rows)| !rows.is_empty()));
    }

    #[test]
    fn the_reader_s_reference_lists_its_keys_then_the_global_ones() {
        let help = Keymap::new().reader_help(false);
        let headings: Vec<&str> = help.iter().map(|(h, _)| *h).collect();
        assert_eq!(
            headings,
            [Scope::Reader.heading(), Scope::ReaderView.heading(), Scope::Global.heading()]
        );
        let reader = &help[0].1;
        assert_eq!(reader[0], ("space / pgdn".to_string(), Action::ReaderNext.label()));
        assert_eq!(reader[1], ("backspace / pgup".to_string(), Action::ReaderPrevious.label()));
        assert_eq!(reader[2], ("← / h".to_string(), Action::ReaderLeft.label()));
        let global = &help[2].1;
        assert!(global.contains(&("q / esc".to_string(), Action::ReaderLeave.label())));
        assert!(global.contains(&("Q / ctrl+c".to_string(), Action::Quit.label())));
        // Section digits and Tab do nothing in the reader, so its reference leaves them out.
        assert!(!help.iter().flat_map(|(_, rows)| rows).any(|(k, _)| k == "tab"));
        let view = &help[1].1;
        assert!(!view.iter().any(|(k, _)| k == "[" || k == "]"), "no width in manga");
        assert!(view.contains(&("z".to_string(), Action::ReaderZoom.label())));
    }

    #[test]
    fn a_novel_s_reference_has_the_width_keys_and_no_zoom() {
        let help = Keymap::new().reader_help(true);
        let view = &help[1].1;
        assert!(view.contains(&("[".to_string(), Action::ReaderNarrower.label())));
        assert!(view.contains(&("]".to_string(), Action::ReaderWider.label())));
        assert!(view.contains(&("m".to_string(), "Paged or scrolled")));
        for gone in [Action::ReaderFit, Action::ReaderZoom, Action::ReaderPages] {
            assert!(!view.iter().any(|(_, label)| *label == gone.label()), "{gone:?} left out");
        }
    }
}
