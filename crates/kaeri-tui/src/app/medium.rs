//! Watch, read or both: the answer the engine keeps for this computer, the question it asks once,
//! and the page that stands in for Home while the answer is reading alone.

use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    medium::{KaeriIsFor, MediumAsk, MediumCommand, MediumView},
};

use super::{App, info};
use crate::nav::Overlay;

/// The three answers, in the order they are offered.
pub const CHOICES: [KaeriIsFor; 3] = [KaeriIsFor::Watch, KaeriIsFor::Read, KaeriIsFor::Both];

/// The word for an answer, as a choice.
pub fn choice_word(answer: KaeriIsFor) -> String {
    match answer {
        KaeriIsFor::Watch => t::medium_watch(),
        KaeriIsFor::Read => t::medium_read(),
        KaeriIsFor::Both => t::medium_both(),
    }
}

/// The line under a choice.
pub fn choice_line(answer: KaeriIsFor) -> String {
    match answer {
        KaeriIsFor::Watch => t::medium_watch_line(),
        KaeriIsFor::Read => t::medium_read_line(),
        KaeriIsFor::Both => t::medium_both_line(),
    }
}

/// The answer as the Settings line says it.
pub fn answer_words(answer: KaeriIsFor) -> String {
    match answer {
        KaeriIsFor::Watch => t::medium_answer_watch(),
        KaeriIsFor::Read => t::medium_answer_read(),
        KaeriIsFor::Both => t::medium_answer_both(),
    }
}

/// A medium about to be turned off, and whether its downloads go with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediumOff {
    pub answer: KaeriIsFor,
    /// Watching is what goes off, else reading.
    pub watching: bool,
    pub delete: bool,
}

impl App {
    /// The engine's word on what Kaeri is for here. A question it carries is asked once.
    pub(super) fn apply_medium(&mut self, view: MediumView) {
        self.medium = Some(view);
        self.open_medium_ask();
    }

    /// Ask the question the engine is holding, when nothing else is open over the screen.
    pub(super) fn open_medium_ask(&mut self) {
        if self.nav.has_overlay() {
            return;
        }
        let Some(ask) = self.medium.as_ref().and_then(|view| view.ask.clone()) else {
            return;
        };
        if self.medium_asked.as_ref() == Some(&ask) {
            return;
        }
        self.medium_asked = Some(ask.clone());
        let chosen = match &ask {
            MediumAsk::Joined { answer, .. } => Some(*answer),
            MediumAsk::Updated => Some(KaeriIsFor::Watch),
            MediumAsk::FirstRun => None,
        };
        self.cursors.overlay.selected =
            chosen.and_then(|a| CHOICES.iter().position(|c| *c == a)).unwrap_or(0);
        self.medium_ask = Some(ask);
        self.nav.open_overlay(Overlay::Medium);
    }

    /// The question the open choice answers, `None` when it is a plain change.
    pub fn medium_question(&self) -> Option<&MediumAsk> {
        self.medium_ask.as_ref()
    }

    /// Whether the question open is the first run's, which cannot be put off.
    pub(super) fn medium_must_answer(&self) -> bool {
        self.nav.overlay() == Some(&Overlay::Medium)
            && matches!(self.medium_ask, Some(MediumAsk::FirstRun))
    }

    /// Whether this computer is set to reading alone, so Home is the page saying where to read.
    pub fn reading_only(&self) -> bool {
        self.medium
            .as_ref()
            .is_some_and(|view| view.reading_available && view.answer == KaeriIsFor::Read)
    }

    /// Whether the answer is this computer's to change.
    pub fn medium_changeable(&self) -> bool {
        self.medium.as_ref().is_some_and(|view| view.reading_available && !view.fixed)
    }

    /// The choice of what Kaeri is for, opened from Settings, the page or the palette.
    pub(super) fn choose_medium(&mut self) {
        if !self.medium_changeable() {
            self.push_toast(info(t::settings_not_editable()));
            return;
        }
        let now = self.medium.as_ref().map_or(KaeriIsFor::Watch, |view| view.answer);
        // The size of the chapters, for the confirmation that may follow.
        self.commands
            .push(Command::Reading(kaeri_model::reading::ReadingCommand::LoadDownloads));
        self.cursors.overlay.selected = CHOICES.iter().position(|c| *c == now).unwrap_or(0);
        self.medium_ask = None;
        self.nav.open_overlay(Overlay::Medium);
    }

    /// Enter in the choice: the highlighted answer, to the engine. A plain change that turns a
    /// medium off asks first, as the apps do.
    pub(super) fn confirm_medium(&mut self) -> Option<Command> {
        let answer = *CHOICES.get(self.cursors.overlay.selected)?;
        let ask = self.medium_ask.take();
        if ask.is_none() {
            let now = self.medium.as_ref().map_or(KaeriIsFor::Watch, |view| view.answer);
            let watching_off = now.watches() && !answer.watches();
            let reading_off = now.reads() && !answer.reads();
            if watching_off || reading_off {
                self.medium_off =
                    Some(MediumOff { answer, watching: watching_off, delete: false });
                self.nav.close_overlay();
                self.cursors.overlay.selected = 1;
                self.nav.open_overlay(Overlay::MediumOff);
                return None;
            }
        }
        self.nav.close_overlay();
        Some(Command::Medium(match ask {
            Some(ask) => MediumCommand::Answer { ask, answer },
            None => MediumCommand::Choose { answer, delete_downloads: false },
        }))
    }

    /// The medium being turned off, while its confirmation is open.
    pub fn medium_off(&self) -> Option<&MediumOff> {
        self.medium_off.as_ref()
    }

    /// What the downloads of the medium being turned off take on this computer: the queue's
    /// files for watching, the chapters the engine last said for reading.
    pub fn medium_off_bytes(&self) -> u64 {
        match &self.medium_off {
            Some(off) if off.watching => self
                .downloads
                .iter()
                .map(|row| {
                    if row.state == kaeri_model::downloads::DownloadState::Done {
                        row.total.max(row.downloaded)
                    } else {
                        row.downloaded
                    }
                })
                .sum(),
            Some(_) => self.book_bytes,
            None => 0,
        }
    }

    /// The chapter downloads' size, from the engine's word on them.
    pub(super) fn take_book_downloads(&mut self, used: u64) {
        self.book_bytes = used;
    }

    /// Enter in the confirmation: the switch flips, or the medium is turned off.
    pub(super) fn confirm_medium_off(&mut self) -> Option<Command> {
        if self.cursors.overlay.selected == 0 {
            if let Some(off) = &mut self.medium_off {
                off.delete = !off.delete;
            }
            return None;
        }
        let off = self.medium_off.take()?;
        self.nav.close_overlay();
        Some(Command::Medium(MediumCommand::Choose {
            answer: off.answer,
            delete_downloads: off.delete,
        }))
    }
}

#[cfg(test)]
mod tests {
    use crate::app::fixtures::*;
    use kaeri_model::medium::{AppEdition, KaeriIsFor, MediumAsk, MediumCommand, MediumView};

    fn view(answer: KaeriIsFor, ask: Option<MediumAsk>) -> MediumView {
        MediumView {
            answer,
            edition: AppEdition::Full,
            fixed: false,
            reading_available: true,
            ask,
            account: None,
        }
    }

    #[test]
    fn the_first_run_asks_once_and_cannot_be_put_off() {
        let mut a = app();
        a.apply(Event::Medium(view(KaeriIsFor::Watch, Some(MediumAsk::FirstRun))));
        assert_eq!(a.nav.overlay(), Some(&Overlay::Medium));
        a.handle(Action::Back, 10);
        assert_eq!(a.nav.overlay(), Some(&Overlay::Medium));
        a.handle(Action::Down, 10);
        let task = a.handle(Action::Open, 10);
        assert_eq!(
            task,
            Some(Command::Medium(MediumCommand::Answer {
                ask: MediumAsk::FirstRun,
                answer: KaeriIsFor::Read
            }))
        );
        assert!(a.nav.overlay().is_none());
        // The same question arriving again is not asked twice.
        a.apply(Event::Medium(view(KaeriIsFor::Read, Some(MediumAsk::FirstRun))));
        assert!(a.nav.overlay().is_none());
    }

    #[test]
    fn a_question_from_another_device_starts_on_its_answer_and_can_be_closed() {
        let mut a = app();
        let ask = MediumAsk::Joined { devices: vec!["Desk".into()], answer: KaeriIsFor::Both };
        a.apply(Event::Medium(view(KaeriIsFor::Watch, Some(ask.clone()))));
        assert_eq!(a.cursors.overlay.selected, 2);
        a.handle(Action::Back, 10);
        assert!(a.nav.overlay().is_none());
    }

    #[test]
    fn a_question_waits_for_the_overlay_in_the_way() {
        let mut a = app();
        a.nav.open_overlay(Overlay::Help);
        a.apply(Event::Medium(view(KaeriIsFor::Watch, Some(MediumAsk::Updated))));
        assert_eq!(a.nav.overlay(), Some(&Overlay::Help));
        a.handle(Action::Back, 10);
        assert_eq!(a.nav.overlay(), Some(&Overlay::Medium));
        assert_eq!(a.cursors.overlay.selected, 0);
    }

    #[test]
    fn reading_alone_puts_the_page_on_home_and_the_key_changes_it() {
        let mut a = app();
        assert!(!a.reading_only());
        a.apply(Event::Medium(view(KaeriIsFor::Read, None)));
        assert!(a.reading_only());
        assert_eq!(a.nav.section(), Section::Home);
        assert_eq!(a.handle(Action::ChooseMedium, 10), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::Medium));
        assert_eq!(a.cursors.overlay.selected, 1);
        a.handle(Action::Down, 10);
        let task = a.handle(Action::Open, 10);
        assert_eq!(
            task,
            Some(Command::Medium(MediumCommand::Choose {
                answer: KaeriIsFor::Both,
                delete_downloads: false
            }))
        );
        // The engine answers with the view in effect, and Home is Home again.
        a.apply(Event::Medium(view(KaeriIsFor::Both, None)));
        assert!(!a.reading_only());
    }

    #[test]
    fn turning_a_medium_off_asks_and_deleting_downloads_is_a_switch_left_off() {
        let mut a = app();
        a.apply(Event::Medium(view(KaeriIsFor::Both, None)));
        a.handle(Action::ChooseMedium, 10);
        // Both to Watch turns reading off.
        a.cursors.overlay.selected = 0;
        assert_eq!(a.handle(Action::Open, 10), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::MediumOff));
        assert_eq!(a.medium_off().map(|o| (o.watching, o.delete)), Some((false, false)));
        // Cancelling sends nothing.
        a.handle(Action::Back, 10);
        assert!(a.nav.overlay().is_none());
        a.handle(Action::ChooseMedium, 10);
        a.cursors.overlay.selected = 0;
        a.handle(Action::Open, 10);
        // Flip the switch, then turn it off.
        a.handle(Action::Up, 10);
        assert_eq!(a.handle(Action::Open, 10), None);
        a.handle(Action::Down, 10);
        assert_eq!(
            a.handle(Action::Open, 10),
            Some(Command::Medium(MediumCommand::Choose {
                answer: KaeriIsFor::Watch,
                delete_downloads: true
            }))
        );
        // Turning one on, or the question of a first run, asks nothing.
        a.apply(Event::Medium(view(KaeriIsFor::Watch, None)));
        a.handle(Action::ChooseMedium, 10);
        a.cursors.overlay.selected = 2;
        assert!(a.handle(Action::Open, 10).is_some());
        // Read to Watch turns reading off; the confirmation says which.
        a.apply(Event::Medium(view(KaeriIsFor::Read, None)));
        a.handle(Action::ChooseMedium, 10);
        a.cursors.overlay.selected = 0;
        a.handle(Action::Open, 10);
        assert_eq!(a.medium_off().map(|o| o.watching), Some(false));
        a.handle(Action::Back, 10);
        a.apply(Event::Medium(view(KaeriIsFor::Both, None)));
        a.handle(Action::ChooseMedium, 10);
        a.cursors.overlay.selected = 1;
        a.handle(Action::Open, 10);
        assert_eq!(a.medium_off().map(|o| o.watching), Some(true));
    }

    #[test]
    fn the_confirmation_knows_what_each_medium_s_downloads_take() {
        use kaeri_model::reading::{BookDownloadsView, ReadingEvent, StorageMeter};
        let mut a = app();
        a.apply(Event::Medium(view(KaeriIsFor::Both, None)));
        a.apply(Event::Reading(ReadingEvent::Downloads(BookDownloadsView {
            meter: StorageMeter { used: 5_000, ..Default::default() },
            ..Default::default()
        })));
        a.handle(Action::ChooseMedium, 10);
        // The chapters are asked for when the choice opens.
        assert!(
            a.take_commands().contains(&Command::Reading(
                kaeri_model::reading::ReadingCommand::LoadDownloads
            ))
        );
        a.cursors.overlay.selected = 0;
        a.handle(Action::Open, 10);
        assert_eq!(a.medium_off_bytes(), 5_000);
        a.handle(Action::Back, 10);
        // Watching off counts the queue's files.
        a.handle(Action::ChooseMedium, 10);
        a.cursors.overlay.selected = 1;
        a.handle(Action::Open, 10);
        assert_eq!(a.medium_off().map(|o| o.watching), Some(true));
        assert_eq!(a.medium_off_bytes(), 0);
    }

    #[test]
    fn a_dark_build_or_a_fixed_edition_shows_nothing_and_cannot_change() {
        let mut a = app();
        let mut dark = view(KaeriIsFor::Watch, None);
        dark.reading_available = false;
        a.apply(Event::Medium(dark));
        assert!(!a.reading_only() && !a.medium_changeable());
        a.handle(Action::ChooseMedium, 10);
        assert!(a.nav.overlay().is_none());
        let mut fixed = view(KaeriIsFor::Read, None);
        fixed.fixed = true;
        fixed.edition = AppEdition::ReadOnly;
        a.apply(Event::Medium(fixed));
        assert!(!a.medium_changeable());
    }
}
