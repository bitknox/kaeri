//! Reading's commands, routed here from the dispatcher.
//!
//! Reading ships dark: until the front end starts the engine with reading on, every reading,
//! reader and book source command is answered with [`ReadingFailure::ReadingOff`] in place, and
//! the medium is Watch whatever is stored. With reading on, the reader opens what is on this
//! device ([`session`]); the loads that have no part behind them yet answer with their empty
//! shapes, and the rest are logged and dropped.

pub(crate) mod eink;
pub mod novel;
pub(crate) mod pages;
pub(crate) mod session;
pub(crate) mod settings;
pub(crate) mod shelf;

pub(crate) use session::Reader;

use kaeri_model::{
    book_sources::BookSourceCommand,
    event::Event,
    medium::{AppEdition, KaeriIsFor, MediumAsk, MediumCommand, MediumView},
    reader::ReaderCommand,
    reading::{
        BookDownloadsView, FeedView, ReadSearchQuery, ReadingAbout, ReadingCommand,
        ReadingEvent, ReadingFailure, ReadingHomeView, UpdatesView,
    },
};

use kaeri_core::{books::WorkId, settings::SettingValue};
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
        account: Option<(&str, Option<String>)>,
        new_install: bool,
    ) -> Self {
        let stored = stored.and_then(KaeriIsFor::parse);
        let (account, writer) = match account {
            Some((answer, writer)) => (KaeriIsFor::parse(answer), writer),
            None => (None, None),
        };
        let ask = match (stored, account) {
            (Some(mine), Some(theirs)) if mine != theirs => Some(MediumAsk::Joined {
                devices: writer.into_iter().collect(),
                answer: theirs,
            }),
            (Some(_), _) => None,
            (None, _) if new_install => Some(MediumAsk::FirstRun),
            (None, _) => Some(MediumAsk::Updated),
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
pub(crate) fn reading(
    engine: &mut EngineState,
    services: &Services,
    command: ReadingCommand,
    tx: &Outbox,
) {
    let about = command.about();
    if !engine.reading.available {
        return off(about, tx);
    }
    let event = match command {
        ReadingCommand::DeviceFacts { .. } | ReadingCommand::AnswerEink { .. } => {
            return eink(engine, services, command, tx);
        }
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
        ReadingCommand::Search { query } => return search(services, query, 0, tx),
        ReadingCommand::SearchMore { query, page } => return search(services, query, page, tx),
        ReadingCommand::LoadChapters { work, edition } => {
            return chapters(services, work, edition, tx);
        }
        ReadingCommand::ChooseEdition { work, source_id, version_label, language } => {
            let pref = kaeri_store::reading::EditionPref { source_id, version_label, language };
            if let Err(e) = services.store.set_edition_pref(work.as_str(), Some(&pref)) {
                tracing::warn!(error = %e, "the edition choice was not kept");
            }
            return chapters(services, work, Some(pref.source_id), tx);
        }
        ReadingCommand::MarkRead { work, chapters: ids, read } => {
            mark_read(services, work, ids, read);
            return;
        }
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

/// Search the book sources, page by page. A page holds what the sources gave: they are asked
/// once, on the first page, and the rest is answered empty.
fn search(services: &Services, query: ReadSearchQuery, page: u32, tx: &Outbox) {
    if page > 0 {
        let _ = tx.send(Event::Reading(ReadingEvent::Search {
            query,
            page,
            tiles: Vec::new(),
            has_more: false,
            other_scope: None,
        }));
        return;
    }
    let (sources, tx, now) = (services.book_sources.clone(), tx.clone(), services.clock.now());
    let plugins = services.book_plugins.clone();
    tokio::spawn(async move {
        sources.adopt_plugins(&plugins);
        let tiles = sources.search(&query.text, query.medium, now).await;
        let _ = tx.send(Event::Reading(ReadingEvent::Search {
            query,
            page: 0,
            tiles,
            has_more: false,
            other_scope: None,
        }));
    });
}

fn chapters(services: &Services, work: WorkId, edition: Option<String>, tx: &Outbox) {
    let (sources, tx, now) = (services.book_sources.clone(), tx.clone(), services.clock.now());
    let plugins = services.book_plugins.clone();
    tokio::spawn(async move {
        sources.adopt_plugins(&plugins);
        let event = match sources.chapters(&work, edition.as_deref(), now).await {
            Ok((edition, volumes)) => {
                ReadingEvent::Chapters { work, edition, volumes, failure: None }
            }
            Err(failure) => ReadingEvent::Chapters {
                work,
                edition: None,
                volumes: Vec::new(),
                failure: Some(failure),
            },
        };
        let _ = tx.send(Event::Reading(event));
    });
}

/// The person's own mark is kept first; the server is told after, and a refusal there is only
/// logged.
fn mark_read(
    services: &Services,
    work: WorkId,
    ids: Vec<kaeri_core::books::ChapterId>,
    read: bool,
) {
    let now = services.clock.now();
    if read {
        for id in &ids {
            let mut event =
                kaeri_store::reading::ReadEvent::new(work.as_str(), id.as_str(), now);
            event.completed = true;
            if let Err(e) = services.store.record_read(&event, Vec::new()) {
                tracing::warn!(error = %e, "a read mark was not kept");
            }
        }
    } else {
        let chapters: Vec<String> = ids.iter().map(|c| c.as_str().to_owned()).collect();
        if let Err(e) = services.store.unread(work.as_str(), &chapters) {
            tracing::warn!(error = %e, "an unread mark was not kept");
        }
    }
    services.book_sources.report(&work, ids, read, now);
}

/// A reader command.
pub(crate) fn reader(
    engine: &mut EngineState,
    services: &Services,
    command: ReaderCommand,
    tx: &Outbox,
) {
    if !engine.reading.available {
        return off(ReadingAbout::Reader, tx);
    }
    let EngineState { config, reader, .. } = engine;
    let mut cx = reader.cx(config, services, tx);
    reader.command(&mut cx, command);
}

/// What Android said about its screen, or the person's answers about it: the profile again.
fn eink(engine: &mut EngineState, services: &Services, command: ReadingCommand, tx: &Outbox) {
    match command {
        ReadingCommand::DeviceFacts { facts } => engine.reader.facts = Some(facts),
        ReadingCommand::AnswerEink { eink, colour } => {
            let word = |v: Option<bool>, yes: &str, no: &str| match v {
                Some(true) => yes.to_owned(),
                Some(false) => no.to_owned(),
                None => "auto".to_owned(),
            };
            for (key, value) in [
                ("mode", word(eink, "on", "off")),
                ("colour_panel", word(colour, "colour", "greys")),
            ] {
                let value = SettingValue::Str(value);
                if let Err(e) =
                    engine.config.write(&services.paths, &["reading", "eink"], key, value)
                {
                    tracing::warn!(error = %e, "the e-ink answer was not written");
                }
            }
        }
        _ => return,
    }
    if let Some(profile) = engine.reader.detect(engine.config.effective()) {
        let _ = tx.send(Event::Reading(ReadingEvent::Eink(profile)));
    }
}

/// A book source command.
pub(crate) fn book_sources(
    state: &ReadingState,
    services: &Services,
    command: BookSourceCommand,
    tx: &Outbox,
) {
    if !state.available {
        return off(ReadingAbout::Sources, tx);
    }
    let stores = services.stores.clone();
    let name_of = move |id: &str| match stores.origins().get(id) {
        Some((kaeri_model::sources::ProviderOrigin::Store { name, .. }, _)) => name.clone(),
        _ => None,
    };
    crate::read_sources::command(
        &services.book_sources,
        &services.book_plugins,
        name_of,
        command,
        services.clock.now(),
        tx,
    );
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
