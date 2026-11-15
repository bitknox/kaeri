//! Private watching and reading: the mode, the titles kept private, and the notice.
//!
//! The engine holds what private mode keeps out of the history, the trackers and sync; the
//! terminal draws it. The mode is said with `Event::Private` at launch and on every change, and
//! is never saved, so a launch starts with it off unless `--private` turns it on. Ctrl+P and the
//! palette turn it over, and so does Enter on its row in Accounts. A title's standing says
//! whether the title is always private on this device and whether its place is held in memory,
//! which the title page's line under the main action reads with the mode as it is now.

use std::collections::HashSet;

use kaeri_core::ids::AnilistId;
use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    private::{PrivateMode, PrivateTitle},
};

use super::App;
use crate::theme::glyph;

/// Idle ticks the notice holds in the status line: 8 s at the loop's 100 ms tick, as long as
/// the apps hold theirs, since it says how to turn the mode off again.
pub const NOTICE_TICKS: u16 = 80;

/// What the terminal knows of private watching and reading.
#[derive(Debug, Default)]
pub(crate) struct Private {
    /// Whether the mode is on, as the engine last said.
    on: bool,
    /// Shows kept always private on this device, as their standing or their flag last said.
    always: HashSet<AnilistId>,
    /// Shows whose next episode and resume point come from a place held in memory.
    kept: HashSet<AnilistId>,
    /// A shortcut or the palette asked for the mode, so the engine's word that it is on brings
    /// the notice. From Accounts the switch's own description is in view, and no notice shows.
    asked: bool,
    /// Idle ticks left on the notice.
    notice: u16,
}

impl Private {
    /// A title's standing, as `Event::Progress` says it.
    pub(super) fn take_standing(
        &mut self,
        id: AnilistId,
        mode: Option<PrivateMode>,
        kept: bool,
    ) {
        self.take_flag(id, mode == Some(PrivateMode::Always));
        if kept {
            self.kept.insert(id);
        } else {
            self.kept.remove(&id);
        }
    }

    /// Whether show `id` is always private, as a staged playback or the flag's change says.
    pub(super) fn take_flag(&mut self, id: AnilistId, always: bool) {
        if always {
            self.always.insert(id);
        } else {
            self.always.remove(&id);
        }
    }
}

/// The line under a title's main action: the head in the state colour, then the rest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateLine {
    pub head: String,
    pub rest: String,
}

impl App {
    /// Whether private mode is on, as the engine last said.
    pub fn private_on(&self) -> bool {
        self.private.on
    }

    /// Whether show `id` is kept always private on this device.
    pub fn always_private(&self, id: AnilistId) -> bool {
        self.private.always.contains(&id)
    }

    /// Whether the title whose page is on screen is kept always private, or `None` off a
    /// title page.
    pub fn always_private_here(&self) -> Option<bool> {
        self.title_on_screen().map(|id| self.always_private(id))
    }

    /// How show `id` is private now, if it is: its own flag first, then the mode.
    pub fn private_mode_of(&self, id: AnilistId) -> Option<PrivateMode> {
        PrivateMode::shown(
            self.always_private(id).then_some(PrivateMode::Always),
            self.private.on,
        )
    }

    /// The line a title page draws under its main action for show `id`, or `None` while
    /// nothing about it is private.
    pub fn private_line(&self, id: AnilistId) -> Option<PrivateLine> {
        let (head, rest) = match self.private_mode_of(id)? {
            PrivateMode::Always => (t::title_always_private(), t::private_line_not_saved()),
            PrivateMode::Session if self.private.kept.contains(&id) => {
                (t::private_badge(), t::private_line_kept())
            }
            PrivateMode::Session => (t::private_badge(), t::private_line_not_saved()),
        };
        Some(PrivateLine { head: format!("{} {}", glyph::PRIVATE, head), rest })
    }

    /// The header chip while the mode is on.
    pub fn private_chip(&self) -> Option<String> {
        self.private.on.then(|| format!("{} {}", glyph::PRIVATE, t::tui_private_chip()))
    }

    /// The notice that takes the status line's place for a while after a shortcut or the
    /// palette turned the mode on.
    pub fn private_notice(&self) -> Option<String> {
        (self.private.notice > 0 && self.private.on)
            .then(|| format!("{} {}", glyph::PRIVATE, t::private_mode_on_notice()))
    }

    /// The mode turned over: asked of the engine, which says when it is in effect. `noticed`
    /// is whether it was asked from a shortcut or the palette, where no description is in view.
    pub(super) fn toggle_private(&mut self, noticed: bool) -> Option<Command> {
        let on = !self.private.on;
        self.private.asked = on && noticed;
        Some(Command::SetPrivate { on })
    }

    /// The title highlighted or on screen kept always private on this device, or not.
    pub(super) fn toggle_always_private(&mut self) -> Option<Command> {
        let id = self.target()?.id;
        Some(Command::SetAlwaysPrivate {
            title: PrivateTitle::Show { id },
            on: !self.always_private(id),
        })
    }

    /// The engine's word on the mode.
    pub(super) fn apply_private(&mut self, on: bool) {
        self.private.on = on;
        self.private.notice =
            if on && std::mem::take(&mut self.private.asked) { NOTICE_TICKS } else { 0 };
        if !on {
            self.private.asked = false;
        }
    }

    /// The engine's word on a title's flag. The terminal has no series page, so a series'
    /// flag has nothing here to change.
    pub(super) fn apply_always_private(&mut self, title: &PrivateTitle, on: bool) {
        if let PrivateTitle::Show { id } = title {
            self.private.take_flag(*id, on);
        }
    }

    /// The notice ages with the toasts.
    pub(super) fn tick_private_notice(&mut self) {
        self.private.notice = self.private.notice.saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use kaeri_model::private::PrivateMode;

    use super::NOTICE_TICKS;
    use crate::app::fixtures::*;

    fn standing(id: u32, private: Option<PrivateMode>, kept: bool) -> Event {
        Event::Progress {
            id: AnilistId::new(id),
            progress: None,
            resume: None,
            episodes: Vec::new(),
            private,
            kept,
        }
    }

    /// The app on title 1's page.
    fn on_a_title() -> App {
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        a.nav.focus_stage();
        a.nav.push(StageView::Title(AnilistId::new(1)));
        a.detail = a.content.entries().first().cloned();
        assert_eq!(a.title_on_screen(), Some(AnilistId::new(1)));
        a
    }

    #[test]
    fn ctrl_p_asks_the_engine_and_the_chip_waits_for_its_word() {
        let mut a = app();
        let ctrl_p = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
        assert_eq!(a.press(ctrl_p, 10), Some(Command::SetPrivate { on: true }));
        assert!(!a.private_on(), "the engine owns the mode");
        assert_eq!(a.private_chip(), None);

        a.apply(Event::Private(true));
        assert!(a.private_on());
        assert_eq!(a.private_chip().as_deref(), Some("◌ private"));
        assert_eq!(a.press(ctrl_p, 10), Some(Command::SetPrivate { on: false }));
        a.apply(Event::Private(false));
        assert_eq!(a.private_chip(), None);
    }

    #[test]
    fn ctrl_p_works_mid_search_and_is_not_typed() {
        let mut a = app();
        a.handle(Action::FocusSearch, 10);
        assert!(a.is_typing());
        let ctrl_p = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
        assert_eq!(a.press(ctrl_p, 10), Some(Command::SetPrivate { on: true }));
        assert_eq!(a.text(Field::Search), "", "never typed as a letter");
    }

    #[test]
    fn a_shortcut_brings_the_notice_and_the_switch_does_not() {
        let mut a = app();
        a.handle(Action::TogglePrivate, 10);
        a.apply(Event::Private(true));
        assert_eq!(
            a.private_notice().as_deref(),
            Some("◌ Private mode is on. Nothing is saved or synced until Kaeri quits.")
        );
        for _ in 0..NOTICE_TICKS - 1 {
            a.tick_toasts();
        }
        assert!(a.private_notice().is_some(), "it holds 8 s");
        a.tick_toasts();
        assert_eq!(a.private_notice(), None);

        // Off and on again from the Accounts row: its description is in view.
        a.apply(Event::Private(false));
        a.handle(Action::ShowAccounts, 10);
        a.nav.focus_stage();
        assert_eq!(a.handle(Action::Open, 10), Some(Command::SetPrivate { on: true }));
        a.apply(Event::Private(true));
        assert_eq!(a.private_notice(), None);
        assert!(a.private_on());
    }

    #[test]
    fn turning_it_off_takes_the_notice_with_it() {
        let mut a = app();
        a.handle(Action::TogglePrivate, 10);
        a.apply(Event::Private(true));
        assert!(a.private_notice().is_some());
        a.apply(Event::Private(false));
        assert_eq!(a.private_notice(), None);
        // Turned on by `--private` at launch, nothing was asked from a shortcut.
        a.apply(Event::Private(true));
        assert_eq!(a.private_notice(), None);
    }

    #[test]
    fn the_accounts_switch_comes_before_the_trackers() {
        let mut a = app();
        a.apply(Event::Sync(Box::new(connected(0))));
        a.handle(Action::ShowAccounts, 10);
        a.nav.focus_stage();
        assert_eq!(a.handle(Action::Open, 10), Some(Command::SetPrivate { on: true }));
        a.apply(Event::Private(true));
        assert_eq!(a.handle(Action::Open, 10), Some(Command::SetPrivate { on: false }));
        a.handle(Action::Down, 10);
        assert_eq!(
            a.handle(Action::Open, 10),
            Some(Command::Disconnect { tracker: "anilist".into() })
        );
    }

    #[test]
    fn the_title_line_reads_the_mode_as_it_is_now() {
        let mut a = on_a_title();
        let id = AnilistId::new(1);
        assert_eq!(a.private_line(id), None, "nothing private");

        a.apply(Event::Private(true));
        let line = a.private_line(id).expect("private now");
        assert_eq!(
            (line.head.as_str(), line.rest.as_str()),
            ("◌ Private", "Not saved or synced")
        );

        a.apply(standing(1, Some(PrivateMode::Session), true));
        let line = a.private_line(id).expect("a place held");
        assert_eq!(line.rest, "Your place is kept until Kaeri quits");

        // Turned off: the place stays in memory, and the page is as it was.
        a.apply(Event::Private(false));
        assert_eq!(a.private_line(id), None);
    }

    #[test]
    fn an_always_private_title_says_so_whatever_the_mode() {
        let mut a = on_a_title();
        let id = AnilistId::new(1);
        a.apply(standing(1, Some(PrivateMode::Always), true));
        for on in [false, true] {
            a.apply(Event::Private(on));
            let line = a.private_line(id).expect("always private");
            assert_eq!(
                (line.head.as_str(), line.rest.as_str()),
                ("◌ Always private", "Not saved or synced"),
                "never the kept line: on={on}"
            );
        }
        // Another show reads the mode alone.
        let other = a.private_line(AnilistId::new(2)).expect("the mode is on");
        assert_eq!(other.head, "◌ Private");
        a.apply(Event::Private(false));
        assert_eq!(a.private_line(AnilistId::new(2)), None);
        assert!(a.private_line(id).is_some(), "still always private with the mode off");
    }

    #[test]
    fn always_private_turns_over_from_the_palette_on_a_title_page() {
        let mut a = on_a_title();
        let id = AnilistId::new(1);
        let listed = |a: &App| {
            a.palette_matches().iter().any(|(action, _)| *action == Action::ToggleAlwaysPrivate)
        };
        a.handle(Action::CommandPalette, 10);
        a.type_char('a');
        a.type_char('l');
        a.type_char('w');
        assert!(listed(&a), "{:?}", a.palette_matches());
        let at = a
            .palette_matches()
            .iter()
            .position(|(action, _)| *action == Action::ToggleAlwaysPrivate)
            .unwrap();
        a.cursors.overlay.selected = at;
        let task = a.handle(Action::Open, 10);
        assert_eq!(
            task,
            Some(Command::SetAlwaysPrivate {
                title: kaeri_model::private::PrivateTitle::Show { id },
                on: true,
            })
        );
        a.apply(Event::AlwaysPrivate {
            title: kaeri_model::private::PrivateTitle::Show { id },
            on: true,
        });
        assert!(a.always_private(id));
        assert_eq!(a.private_line(id).map(|l| l.head), Some("◌ Always private".into()));
        assert_eq!(
            a.handle(Action::ToggleAlwaysPrivate, 10),
            Some(Command::SetAlwaysPrivate {
                title: kaeri_model::private::PrivateTitle::Show { id },
                on: false,
            })
        );

        // Off the title page the palette leaves it out.
        a.handle(Action::Back, 10);
        a.handle(Action::CommandPalette, 10);
        a.type_char('a');
        a.type_char('l');
        a.type_char('w');
        assert!(!listed(&a), "{:?}", a.palette_matches());
    }

    #[test]
    fn a_staged_play_says_whether_its_show_is_always_private() {
        let mut a = app();
        a.apply(Event::PlaybackStaged {
            id: AnilistId::new(7),
            title: "Title 7".into(),
            episode: "1".into(),
            episode_title: None,
            speed: 1.0,
            resume: None,
            always_private: true,
        });
        assert!(a.always_private(AnilistId::new(7)));
        assert_eq!(a.private_mode_of(AnilistId::new(7)), Some(PrivateMode::Always));
    }

    #[test]
    fn a_series_flag_leaves_the_shows_alone() {
        let mut a = app();
        a.apply(Event::AlwaysPrivate {
            title: kaeri_model::private::PrivateTitle::Series {
                work: kaeri_core::books::WorkId::new("a:1"),
            },
            on: true,
        });
        assert!(!a.always_private(AnilistId::new(1)));
    }
}
