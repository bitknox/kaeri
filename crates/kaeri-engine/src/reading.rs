//! Reading's commands, routed here from the dispatcher.
//!
//! Reading ships dark: until the front end starts the engine with reading on, every reading,
//! reader and book source command is answered with [`ReadingFailure::ReadingOff`] in place, and
//! the medium is Watch whatever is stored. With reading on, the loads answer with their empty
//! shapes until the parts that fill them land; the rest are logged and dropped.

use kaeri_model::{
    book_sources::{BookSourceCommand, BookSourceEvent, BookSourcesView},
    event::Event,
    medium::{AppEdition, KaeriIsFor, MediumAsk, MediumCommand, MediumView},
    reader::{ReaderCommand, ReaderEvent, ReaderSettingsView},
    reading::{
        BookDownloadsView, FeedView, ReadingAbout, ReadingCommand, ReadingEvent,
        ReadingFailure, ReadingHomeView, UpdatesView,
    },
};

use kaeri_core::settings::SettingValue;
use kaeri_store::sync::kinds;

use crate::{outbox::Outbox, services::Services, state::EngineState};

/// What the engine knows about reading on this device.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadingState {
    /// Reading is in this build: false while it ships dark.
    pub available: bool,
    pub edition: AppEdition,
    /// This device's own answer, as kept.
    pub stored: Option<KaeriIsFor>,
    /// The account's answer, from sync.
    pub account: Option<KaeriIsFor>,
    /// A question still to show once.
    pub ask: Option<MediumAsk>,
}

impl ReadingState {
    pub fn new(available: bool, edition: AppEdition) -> Self {
        Self { available, edition, ..Self::default() }
    }

    /// The state at launch: this device's kept answer, the account's from sync, and whether
    /// the install is new. A device with no answer is asked once: on a new install as the
    /// second page of the first run, after an update as one sheet over Home. When the account
    /// says otherwise, the question names the devices that do.
    pub fn at_launch(
        available: bool,
        edition: AppEdition,
        stored: Option<&str>,
        account: Option<&str>,
        new_install: bool,
    ) -> Self {
        let stored = stored.and_then(KaeriIsFor::parse);
        let account = account.and_then(KaeriIsFor::parse);
        let ask = match stored {
            Some(_) => None,
            None if new_install => Some(MediumAsk::FirstRun),
            None => Some(MediumAsk::Updated),
        };
        Self { available, edition, stored, account, ask }
    }

    /// The answer in effect: Watch while reading is not available, else the edition's own.
    pub fn answer(&self) -> KaeriIsFor {
        MediumView::effective(self.stored, self.edition, self.available)
    }

    /// Whether reading shows anywhere on this device.
    pub fn reads(&self) -> bool {
        self.available && self.answer().reads()
    }

    pub fn view(&self) -> MediumView {
        let fixed = self.edition.fixed().is_some();
        MediumView {
            answer: self.answer(),
            edition: self.edition,
            fixed,
            reading_available: self.available,
            // Never asked while dark, nor where the edition fixes the answer.
            ask: if self.available && !fixed { self.ask.clone() } else { None },
            account: self.account,
        }
    }

    /// Whether this device's answer is its own to give.
    fn open(&self) -> bool {
        self.available && self.edition.fixed().is_none()
    }

    /// The account's answer arrived from another device, named `writer`. Asks when it differs
    /// from this device's, and says whether the view changed.
    pub fn account_received(
        &mut self,
        answer: Option<KaeriIsFor>,
        writer: Option<String>,
    ) -> bool {
        let before = self.view();
        self.account = answer;
        if let Some(answer) = answer
            && self.open()
            && answer != self.answer()
        {
            let devices = writer.into_iter().collect();
            self.ask = Some(MediumAsk::Joined { devices, answer });
        }
        self.view() != before
    }
}

/// Watch, read or both: always answered with the view. The answer is this device's, kept in
/// `reading.kaeri_is_for`, and told to the account so a device that joins later is asked.
pub(crate) fn medium(
    engine: &mut EngineState,
    services: &Services,
    command: MediumCommand,
    tx: &Outbox,
) {
    let open = engine.reading.open();
    let answer = match command {
        MediumCommand::Load => None,
        MediumCommand::Choose { answer, delete_downloads } if open => {
            if delete_downloads {
                // No chapters are downloaded until downloads land.
                tracing::debug!("no chapter downloads to delete");
            }
            Some(answer)
        }
        MediumCommand::Answer { ask: _, answer } if open => {
            engine.reading.ask = None;
            Some(answer)
        }
        MediumCommand::Choose { .. } | MediumCommand::Answer { .. } => {
            tracing::debug!("the medium is fixed in this build");
            None
        }
    };
    if let Some(answer) = answer {
        engine.reading.stored = Some(answer);
        let value = SettingValue::Str(answer.as_str().to_owned());
        if let Err(e) =
            engine.config.write(&services.paths, &["reading"], "kaeri_is_for", value)
        {
            tracing::warn!(error = %e, "the medium answer was not written");
        }
        if let Err(e) = services.store.sync_register(
            kinds::MEDIUM_ACCOUNT,
            "account",
            Some(answer.as_str()),
            None,
        ) {
            tracing::warn!(error = %e, "the medium answer was not told to sync");
        }
        engine.reading.account = Some(answer);
    }
    let _ = tx.send(Event::Medium(engine.reading.view()));
}

/// A reading command.
pub(crate) fn reading(state: &ReadingState, command: ReadingCommand, tx: &Outbox) {
    let about = command.about();
    if !state.available {
        return off(about, tx);
    }
    let event = match command {
        ReadingCommand::LoadHome { .. } => ReadingEvent::Home(Box::new(ReadingHomeView {
            no_sources: true,
            ..Default::default()
        })),
        ReadingCommand::LoadUpdates | ReadingCommand::CheckUpdates => {
            ReadingEvent::Updates(UpdatesView::default())
        }
        ReadingCommand::LoadLibrary { segment } => {
            ReadingEvent::Library { segment, tiles: Vec::new() }
        }
        ReadingCommand::Search { query } => ReadingEvent::Search {
            query,
            page: 0,
            tiles: Vec::new(),
            has_more: false,
            other_scope: None,
        },
        ReadingCommand::SearchMore { query, page } => ReadingEvent::Search {
            query,
            page,
            tiles: Vec::new(),
            has_more: false,
            other_scope: None,
        },
        ReadingCommand::LoadDownloads => ReadingEvent::Downloads(BookDownloadsView::default()),
        ReadingCommand::LoadExportTargets => ReadingEvent::ExportTargets(Vec::new()),
        ReadingCommand::LoadFeed => ReadingEvent::Feed(FeedView::default()),
        other => {
            tracing::debug!(?about, command = ?other, "reading command not handled yet");
            return;
        }
    };
    let _ = tx.send(Event::Reading(event));
}

/// A reader command.
pub(crate) fn reader(state: &ReadingState, command: ReaderCommand, tx: &Outbox) {
    if !state.available {
        return off(ReadingAbout::Reader, tx);
    }
    match command {
        ReaderCommand::LoadSettings { medium, work } => {
            let view = ReaderSettingsView { medium, work, rows: Vec::new() };
            let _ = tx.send(Event::Reader(ReaderEvent::Settings(view)));
        }
        ReaderCommand::Close { session } => {
            let _ = tx.send(Event::Reader(ReaderEvent::Closed { session }));
        }
        other => tracing::debug!(command = ?other, "reader command not handled yet"),
    }
}

/// A book source command.
pub(crate) fn book_sources(state: &ReadingState, command: BookSourceCommand, tx: &Outbox) {
    if !state.available {
        return off(ReadingAbout::Sources, tx);
    }
    match command {
        BookSourceCommand::Load => {
            let view = BookSourcesView::default();
            let _ = tx.send(Event::BookSources(BookSourceEvent::View(view)));
        }
        other => tracing::debug!(command = ?other, "book source command not handled yet"),
    }
}

fn off(about: ReadingAbout, tx: &Outbox) {
    let failure = ReadingFailure::ReadingOff;
    let _ = tx.send(Event::Reading(ReadingEvent::Failed { about, failure }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_reading_is_watch_and_never_asks() {
        let mut state = ReadingState::new(false, AppEdition::Full);
        state.stored = Some(KaeriIsFor::Both);
        state.ask = Some(MediumAsk::Updated);
        let view = state.view();
        assert_eq!(view.answer, KaeriIsFor::Watch);
        assert!(!view.reading_available && view.ask.is_none() && !state.reads());

        let mut on = ReadingState::new(true, AppEdition::Full);
        on.ask = Some(MediumAsk::FirstRun);
        assert_eq!(on.view().ask, Some(MediumAsk::FirstRun));
        on.stored = Some(KaeriIsFor::Both);
        assert!(on.reads());

        let tv = ReadingState::new(true, AppEdition::WatchOnly);
        assert!(tv.view().fixed && !tv.reads());
        let reader = ReadingState {
            ask: Some(MediumAsk::FirstRun),
            ..ReadingState::new(true, AppEdition::ReadOnly)
        };
        assert_eq!(reader.view().answer, KaeriIsFor::Read);
        assert_eq!(reader.view().ask, None);
    }

    #[test]
    fn a_device_without_an_answer_is_asked_once_and_the_account_can_ask_again() {
        let fresh = ReadingState::at_launch(true, AppEdition::Full, None, None, true);
        assert_eq!(fresh.view().ask, Some(MediumAsk::FirstRun));
        let updated = ReadingState::at_launch(true, AppEdition::Full, None, None, false);
        assert_eq!(updated.view().ask, Some(MediumAsk::Updated));
        let mut kept =
            ReadingState::at_launch(true, AppEdition::Full, Some("both"), None, false);
        assert_eq!((kept.view().ask, kept.answer()), (None, KaeriIsFor::Both));

        kept.account_received(Some(KaeriIsFor::Both), Some("Desk".into()));
        assert_eq!(kept.view().ask, None);
        assert!(kept.account_received(Some(KaeriIsFor::Read), Some("Desk".into())));
        assert_eq!(
            kept.view().ask,
            Some(MediumAsk::Joined { devices: vec!["Desk".into()], answer: KaeriIsFor::Read })
        );

        let mut dark = ReadingState::at_launch(false, AppEdition::Full, None, None, true);
        dark.account_received(Some(KaeriIsFor::Read), None);
        assert_eq!(dark.view().ask, None);
    }
}
