//! Reading's commands, routed here from the dispatcher.
//!
//! Every app starts the engine with reading on. Started with it off, as a test or a still frame
//! may be, every reading, reader and book source command is answered with
//! [`ReadingFailure::ReadingOff`] in place, and the medium is Watch whatever is stored. With
//! reading on, the reader opens what is on this device ([`session`]); the loads that have no part
//! behind them yet answer with their empty shapes, and the rest are logged and dropped.

pub(crate) mod credits;
pub(crate) mod eink;
pub(crate) mod extras;
pub(crate) mod feed;
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
        LibrarySegmentRead, ReadSearchQuery, ReadingAbout, ReadingCommand, ReadingEvent,
        ReadingFailure, ReadingHomeView, UpdatesView,
    },
};

use kaeri_core::{books::WorkId, settings::SettingValue};
use kaeri_store::sync::kinds;

use crate::{outbox::Outbox, services::Services, state::EngineState};

/// What the engine knows about reading on this device.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadingState {
    /// Reading is on: false only where the engine was started with it off.
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
            // Never asked with reading off, nor where the edition fixes the answer.
            ask: if self.available && !fixed { self.ask.clone() } else { None },
            account: self.account,
        }
    }

    /// Whether synced reading entries (chapter progress, book source choices) are acted on here.
    /// The TV stores and relays them untouched and does nothing with them.
    pub fn takes_synced_reading(&self) -> bool {
        self.edition != AppEdition::WatchOnly
    }

    /// Whether this synced register kind is a reading entry this device ignores.
    pub fn ignores_synced(&self, kind: &str) -> bool {
        !self.takes_synced_reading()
            && matches!(kind, kinds::BOOK_SOURCE_ORDER | kinds::BOOK_SOURCE_OFF)
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
            if delete_downloads && !answer.reads() {
                // Reading is the medium turned off: its downloads go.
                let lent = crate::downloads::books::Lent::of(services, tx);
                let gone = services.book_sources.downloads.delete_all(&lent);
                tracing::debug!(gone, "chapter downloads deleted with the medium");
            }
            if delete_downloads && !answer.watches() {
                // Watching is the medium turned off: its episode downloads go.
                let gone = crate::downloads::commands::download_delete_all(services, tx);
                tracing::debug!(gone, "episode downloads deleted with the medium");
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
    services.syncing.set_medium(engine.reading.answer());
    let _ = tx.send(Event::Medium(engine.reading.view()));
    // Accounts lists the trackers that serve what Kaeri is for.
    if answer.is_some() {
        for state in services.sync.states_shown(engine.reading.answer()) {
            let _ = tx.send(Event::Sync(Box::new(state)));
        }
    }
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
            return chapters(engine, services, work, edition, tx);
        }
        ReadingCommand::LoadVersions { work, chapter } => {
            let versions = other_sources(services, &work, &chapter);
            ReadingEvent::Versions { work, chapter, versions }
        }
        ReadingCommand::ChooseEdition { work, source_id, version_label, language } => {
            let pref = kaeri_store::reading::EditionPref { source_id, version_label, language };
            if let Err(e) = services.store.set_edition_pref(work.as_str(), Some(&pref)) {
                tracing::warn!(error = %e, "the edition choice was not kept");
            }
            return chapters(engine, services, work, Some(pref.source_id), tx);
        }
        ReadingCommand::ChooseGroup { work, group } => {
            choose_group(services, &work, group);
            let ask = crate::read_catalog::Ask::of(engine, services);
            let kit = crate::read_catalog::Kit::of(services);
            chapters(engine, services, work.clone(), None, tx);
            crate::read_catalog::series::header(&kit, ask, &work, tx);
            return;
        }
        ReadingCommand::MarkRead { work, chapters: ids, read } => {
            mark_read(services, work, ids, read, tx);
            return;
        }
        ReadingCommand::MarkReadUpTo { .. }
        | ReadingCommand::ReadAgain { .. }
        | ReadingCommand::UndoReadAgain { .. }
        | ReadingCommand::Bookmark { .. } => {
            let now = services.clock.now();
            let (work, read) = match &command {
                ReadingCommand::MarkReadUpTo { work, .. } => (work.clone(), true),
                ReadingCommand::ReadAgain { work } | ReadingCommand::UndoReadAgain { work } => {
                    (work.clone(), false)
                }
                ReadingCommand::Bookmark { work, .. } => (work.clone(), false),
                _ => return,
            };
            let again = match &command {
                ReadingCommand::ReadAgain { .. } => Some(true),
                ReadingCommand::UndoReadAgain { .. } => Some(false),
                _ => None,
            };
            if let Err(e) = marks(&services.store, command, now) {
                tracing::warn!(error = %e, "a reading mark was not kept");
                return;
            }
            if let Some(again) = again {
                rereading(services, &work, again, now, tx);
            }
            if read {
                let lent = crate::downloads::books::Lent::of(services, tx);
                services.book_sources.downloads.sweep(&lent, Some(work.as_str()));
            }
            let _ = crate::read_tracking::progress_of(services, &work);
            return;
        }
        ReadingCommand::LoadDownloads => {
            let view = services.book_sources.downloads.view(&services.store);
            let works: Vec<String> = view.series.iter().map(|s| s.work.0.clone()).collect();
            let _ = tx.send(Event::Reading(ReadingEvent::Downloads(view)));
            downloads_books(services, &works, tx);
            return;
        }
        ReadingCommand::Download { .. }
        | ReadingCommand::DownloadCancel { .. }
        | ReadingCommand::DownloadDelete { .. }
        | ReadingCommand::DownloadDeleteRead { .. }
        | ReadingCommand::EstimateDownload { .. }
        | ReadingCommand::DownloadNew => {
            return downloads(engine, services, command, tx);
        }
        other => {
            tracing::debug!(?about, command = ?other, "reading command not handled yet");
            return;
        }
    };
    let _ = tx.send(Event::Reading(event));
}

/// Downloads again once the books of series nothing is kept of land, so their rows get their
/// covers and titles though Downloads was the first screen asked.
fn downloads_books(services: &Services, works: &[String], tx: &Outbox) {
    let meta = services.meta.clone();
    let unkept = crate::read_catalog::tiles::unkept(&meta, works);
    if unkept.is_empty() {
        return;
    }
    let (queue, store, tx) =
        (services.book_sources.downloads.clone(), services.store.clone(), tx.clone());
    tokio::spawn(async move {
        match kaeri_meta::on_screen(meta.books_many(&unkept)).await {
            Ok(_) => {
                let _ = tx.send(Event::Reading(ReadingEvent::Downloads(queue.view(&store))));
            }
            Err(e) => {
                tracing::info!(error = %e, "the books behind Downloads were not asked for")
            }
        }
    });
}

/// A download command: the queue is set up from the config and the network first, so what it
/// does follows what the person last chose.
fn downloads(
    engine: &mut EngineState,
    services: &Services,
    command: ReadingCommand,
    tx: &Outbox,
) {
    use crate::downloads::books::{Lent, Setup};
    let queue = services.book_sources.downloads.clone();
    queue.configure(Setup::of(
        engine.phase,
        engine.said_network(),
        engine.config.effective(),
        engine.reading.edition,
    ));
    let lent = Lent::of(services, tx);
    match command {
        ReadingCommand::Download { work, pick } => {
            tokio::spawn(async move { queue.download(&lent, work, pick).await });
        }
        ReadingCommand::DownloadCancel { work, chapters } => {
            queue.cancel(&lent, &work, &chapters)
        }
        ReadingCommand::DownloadDelete { work, chapters } => {
            queue.delete(&lent, &work, &chapters);
        }
        ReadingCommand::DownloadDeleteRead { work } => {
            queue.delete_read(&lent, &work);
        }
        ReadingCommand::EstimateDownload { work, pick } => {
            let kit = crate::read_catalog::Kit::of(services);
            let (edition, volumes) = crate::read_catalog::series::kept_volumes(&kit, &work);
            let rows = crate::downloads::books::flatten(volumes);
            let source = edition.map(|e| e.source_id);
            let estimate =
                queue.estimate(&services.store, &work, pick, &rows, source.as_deref());
            let _ = tx.send(Event::Reading(ReadingEvent::DownloadEstimate(estimate)));
        }
        ReadingCommand::DownloadNew => {
            let ask = crate::read_catalog::Ask::of(engine, services);
            let kit = crate::read_catalog::Kit::of(services);
            tokio::spawn(async move { queue.download_new(&kit, ask, &lent).await });
        }
        _ => {}
    }
}

/// The other sources a chapter could be read from. A file on the shelf is read only from that
/// file, so it has none; a chapter listed from a source can be read from any other source that
/// holds the work, which lists its chapters again once chosen.
fn other_sources(
    services: &Services,
    work: &WorkId,
    chapter: &kaeri_core::books::ChapterId,
) -> Vec<kaeri_model::reading::ChapterVersion> {
    let on_shelf = shelf::chapters(&services.store, work.as_str())
        .iter()
        .any(|c| &c.id == chapter && c.format != session::SOURCE);
    if on_shelf {
        return Vec::new();
    }
    let sources = &services.book_sources;
    let current = sources.edition_of(work, None).map(|(source, _)| source);
    sources
        .editions(work)
        .into_iter()
        .filter(|e| current.as_ref() != Some(&e.source_id))
        .map(|edition| kaeri_model::reading::ChapterVersion {
            edition,
            chapter_ref: chapter.as_str().to_owned(),
            published: None,
        })
        .collect()
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
            total: None,
        }));
        return;
    }
    let (sources, tx, now) = (services.book_sources.clone(), tx.clone(), services.clock.now());
    let plugins = services.book_plugins.clone();
    tokio::spawn(async move {
        sources.adopt_plugins(&plugins);
        let scope = query.source_scope.as_deref();
        let tiles = sources.search_in(&query.text, query.medium, scope, now).await;
        let _ = tx.send(Event::Reading(ReadingEvent::Search {
            query,
            page: 0,
            tiles,
            has_more: false,
            other_scope: None,
            total: None,
        }));
    });
}

/// A work's chapters for the reader's sheet and the series page. A series no source holds lists
/// its files on this device; one with neither answers why.
fn chapters(
    engine: &EngineState,
    services: &Services,
    work: WorkId,
    edition: Option<String>,
    tx: &Outbox,
) {
    let kit = crate::read_catalog::Kit::of(services);
    let ask = crate::read_catalog::Ask::of(engine, services);
    chapters_of(kit, ask, work, edition, tx);
}

/// [`chapters`] with the catalogue's handles, as the series page's tests drive it.
pub(crate) fn chapters_of(
    kit: crate::read_catalog::Kit,
    ask: crate::read_catalog::Ask,
    work: WorkId,
    edition: Option<String>,
    tx: &Outbox,
) {
    let tx = tx.clone();
    tokio::spawn(async move {
        crate::read_catalog::series::chapters(&kit, ask, &work, edition, &tx).await;
    });
}

/// Keep the person's group for the edition in use. The group is the edition's version, so the
/// pick is the edition choice with its version set: a choice of another source is replaced by
/// the source in use.
fn choose_group(services: &Services, work: &WorkId, group: Option<String>) {
    let Some((source, _)) = services.book_sources.edition_of(work, None) else { return };
    let kept = services.store.edition_pref(work.as_str()).ok().flatten();
    let pref = match kept {
        Some(mut pref) if pref.source_id == source => {
            pref.version_label = group;
            pref
        }
        _ => kaeri_store::reading::EditionPref {
            source_id: source,
            version_label: group,
            language: None,
        },
    };
    if let Err(e) = services.store.set_edition_pref(work.as_str(), Some(&pref)) {
        tracing::warn!(error = %e, "the group choice was not kept");
    }
}

/// The person's own mark is kept first; the server is told after, and a refusal there is only
/// logged.
fn mark_read(
    services: &Services,
    work: WorkId,
    ids: Vec<kaeri_core::books::ChapterId>,
    read: bool,
    tx: &Outbox,
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
    if read {
        let lent = crate::downloads::books::Lent::of(services, tx);
        services.book_sources.downloads.sweep(&lent, Some(work.as_str()));
    }
    services.book_sources.report(&work, ids, read, now);
    // What the trackers hold follows the catalogue.
    let progress = crate::read_tracking::progress_of(services, &work);
    // When the latest change wins, the trackers come down with an unread too.
    if !read && services.sync.read_rule() == kaeri_core::config::WhenTrackersDisagree::Latest {
        crate::read_tracking::queue_lower(services, &work, progress, now, tx);
    }
}

/// Read again puts the series on Rereading here and on the trackers, which keep their chapter
/// count; its undo puts back the list the series was on.
fn rereading(services: &Services, work: &WorkId, again: bool, now: i64, tx: &Outbox) {
    let store = &services.store;
    let key = format!("read_again_was:{}", work.as_str());
    let status = if again {
        let was = store.read_status(work.as_str()).ok().and_then(|(s, _)| s);
        if was.as_deref() == Some(LibrarySegmentRead::Rereading.as_str()) {
            return;
        }
        if let Err(e) = store.set_meta_text(&key, was.as_deref().unwrap_or("")) {
            tracing::warn!(error = %e, "the list before Read again was not kept");
        }
        Some(LibrarySegmentRead::Rereading)
    } else {
        let Ok(Some(was)) = store.get_meta_text(&key) else { return };
        let _ = store.delete_meta(&key);
        LibrarySegmentRead::parse(&was)
    };
    let name = status.map(LibrarySegmentRead::as_str);
    if let Err(e) = store.set_read_status(work.as_str(), name, now) {
        tracing::warn!(error = %e, "the reading list was not kept");
    }
    if let Some(status) = status {
        crate::read_tracking::queue_status(services, work, status, now, tx);
    }
}

/// Marks on many chapters at once, and bookmarks, kept in the store. Like [`mark_read`] they
/// send nothing; the app asks for the series and its chapters again.
fn marks(
    store: &kaeri_store::Store,
    command: ReadingCommand,
    now: i64,
) -> kaeri_store::Result<()> {
    match command {
        ReadingCommand::MarkReadUpTo { work, chapter } => {
            store.adopt_read(work.as_str(), chapter.as_str(), None, now)
        }
        ReadingCommand::ReadAgain { work } => store.reset_read(work.as_str(), now).map(drop),
        ReadingCommand::UndoReadAgain { work } => {
            match store.read_reset_in_force(work.as_str())? {
                Some(reset) => store.undo_read_reset(work.as_str(), &reset, now),
                None => Ok(()),
            }
        }
        ReadingCommand::Bookmark { work, chapter, on } => {
            store.set_bookmark(work.as_str(), chapter.as_str(), on, now)
        }
        _ => Ok(()),
    }
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
            // The profile below reads the config in effect, which a write leaves alone: the
            // file's answers go in effect too, or the switch would only take on the next start.
            let answers = engine.config.saved().reading.eink.clone();
            engine.config.effective_mut().reading.eink = answers;
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

pub(crate) fn off(about: ReadingAbout, tx: &Outbox) {
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
        assert!(!tv.takes_synced_reading());
        assert!(tv.ignores_synced(kinds::BOOK_SOURCE_ORDER));
        assert!(tv.ignores_synced(kinds::BOOK_SOURCE_OFF));
        assert!(!tv.ignores_synced(kinds::MEDIUM_ACCOUNT));
        assert!(on.takes_synced_reading() && !on.ignores_synced(kinds::BOOK_SOURCE_OFF));
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

    #[test]
    fn up_to_here_read_again_and_its_undo_and_bookmarks_are_kept() {
        use kaeri_core::books::ChapterId;
        let store = kaeri_store::Store::open_in_memory().unwrap();
        let work = WorkId::new("a:900301");
        let read = |chapter: &str| store.chapter_is_read("a:900301", chapter).unwrap();

        let up_to =
            ReadingCommand::MarkReadUpTo { work: work.clone(), chapter: ChapterId::new("12") };
        marks(&store, up_to, 10).unwrap();
        assert!(read("1") && read("12") && !read("13"));

        marks(&store, ReadingCommand::ReadAgain { work: work.clone() }, 20).unwrap();
        assert!(!read("1") && !read("12"));
        marks(&store, ReadingCommand::UndoReadAgain { work: work.clone() }, 30).unwrap();
        assert!(read("1") && read("12"));
        // Nothing to undo leaves the marks as they are.
        marks(&store, ReadingCommand::UndoReadAgain { work: work.clone() }, 40).unwrap();
        assert!(read("12"));

        let on = ReadingCommand::Bookmark {
            work: work.clone(),
            chapter: ChapterId::new("7"),
            on: true,
        };
        marks(&store, on, 50).unwrap();
        assert_eq!(store.bookmarks("a:900301").unwrap(), vec!["7".to_owned()]);
        let off = ReadingCommand::Bookmark { work, chapter: ChapterId::new("7"), on: false };
        marks(&store, off, 60).unwrap();
        assert!(store.bookmarks("a:900301").unwrap().is_empty());
    }
}
