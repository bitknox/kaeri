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

use crate::outbox::Outbox;

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

    /// Watch, read or both: always answered with the view.
    pub fn medium(&mut self, command: MediumCommand, tx: &Outbox) {
        let open = self.available && self.edition.fixed().is_none();
        match command {
            MediumCommand::Load => {}
            MediumCommand::Choose { answer, delete_downloads: _ } if open => {
                self.stored = Some(answer);
            }
            MediumCommand::Answer { ask: _, answer } if open => {
                self.stored = Some(answer);
                self.ask = None;
            }
            MediumCommand::Choose { .. } | MediumCommand::Answer { .. } => {
                tracing::debug!("the medium is fixed in this build");
            }
        }
        let _ = tx.send(Event::Medium(self.view()));
    }
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
}
