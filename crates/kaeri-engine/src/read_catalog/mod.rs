//! Reading's catalogue: Home, Updates, the library lists, search, the series page and its
//! versions, from AniList's books and the person's own sources.
//!
//! The catalogue follows the shows' rules. A launch asks AniList for nothing but the first Home;
//! everything else waits for [`kaeri_meta::AniList::launched`] and goes in the paced background
//! queue. Every AniList request a screen needs goes as one, with its parts under aliases.

pub(crate) mod arrange;
pub(crate) mod changed;
pub(crate) mod chapters;
pub(crate) mod discovery;
pub(crate) mod editions;
pub(crate) mod extras;
pub(crate) mod fit;
pub(crate) mod home;
pub(crate) mod rows;
pub(crate) mod search;
pub(crate) mod series;
#[cfg(test)]
mod tests;
pub(crate) mod tiles;
pub(crate) mod updates;
pub(crate) mod volumes;

use std::sync::{Arc, Mutex, atomic::AtomicBool};

use kaeri_model::{
    event::Event,
    medium::KaeriIsFor,
    reading::{ReadingCommand, ReadingEvent, ReadingFailure},
};

use crate::{outbox::Outbox, services::Services, state::EngineState};

/// What the catalogue keeps between commands. Cheap to clone; every clone shares it.
#[derive(Debug, Clone, Default)]
pub struct Memory {
    /// A check of followed series is running.
    pub(crate) checking: Arc<AtomicBool>,
    /// One check at a time.
    pub(crate) check_turn: Arc<tokio::sync::Mutex<()>>,
    /// Home parts on their way.
    pub(crate) home: Arc<Mutex<home::Loading>>,
    /// The series the last check could not ask about.
    pub(crate) failures: Arc<Mutex<Vec<kaeri_model::reading::UpdateFailure>>>,
    /// The sources searched for a series without a match, for "Not found in".
    pub(crate) misses: Arc<Mutex<editions::Misses>>,
    /// The search with words whose sources are being asked.
    pub(crate) search: Arc<Mutex<search::Run>>,
    /// The words and filters last searched, and their generation: AniList's answers for older
    /// ones are dropped.
    pub(crate) searched: Arc<Mutex<search::Searched>>,
    /// The series opened that no source had, with when that was found and the
    /// [`crate::read_sources::BookSources::sources_mark`] of the sources asked: a Home tile says
    /// so for the rest of its row's 30 minutes, while those sources stand.
    pub(crate) not_found: Arc<Mutex<std::collections::HashMap<String, (i64, u64)>>>,
    /// The works whose sources are being searched for them by name, each ending when the
    /// search does: a page waits for it before it says no source has the series.
    pub(crate) checks: Arc<Mutex<std::collections::HashMap<String, editions::Check>>>,
    /// Each edition's last chapter-list ask that left no list kept: being asked, answered
    /// empty, or failed, with when.
    pub(crate) listings: Listings,
    /// The chapter lists on their way to a page, by work and edition asked for: a second ask
    /// meanwhile hears the first one's answer rather than asking the source again.
    pub(crate) chapter_loads: ChapterLoads,
    /// What is on screen, to bring up to date when the sources change: see [`changed`].
    pub(crate) shown: Arc<Mutex<changed::Shown>>,
}

/// Each edition's last chapter-list ask that left no list kept, by source and series, with when
/// and the [`crate::read_sources::BookSources::source_mark`] of the source asked.
pub(crate) type Listings = Arc<
    Mutex<std::collections::HashMap<(String, String), (i64, chapters::Asked, Option<u64>)>>,
>;

/// The chapter lists on their way to a page, by work and the edition asked for.
pub(crate) type ChapterLoads = Arc<Mutex<std::collections::HashSet<(String, Option<String>)>>>;

/// The handles the catalogue's tasks are lent, cloned.
#[derive(Clone)]
pub(crate) struct Kit {
    pub meta: kaeri_meta::Meta,
    pub store: kaeri_store::Store,
    pub sources: crate::read_sources::BookSources,
    pub plugins: crate::book_plugins::BookPlugins,
    pub clock: crate::clock::Clock,
    pub offline: Arc<AtomicBool>,
}

impl Kit {
    pub fn of(services: &Services) -> Self {
        Self {
            meta: services.meta.clone(),
            store: services.store.clone(),
            sources: services.book_sources.clone(),
            plugins: services.book_plugins.clone(),
            clock: services.clock.clone(),
            offline: services.offline.clone(),
        }
    }

    /// Whether the person has no book source of their own yet: no folder, no server, no
    /// plugin.
    pub fn no_sources(&self) -> bool {
        let folders = self.store.book_folders().map(|f| f.is_empty()).unwrap_or(true);
        let others = self.sources.registry.ids().iter().any(|id| id != "folders");
        folders && !others
    }
}

/// What a catalogue command needs to know of the engine's state, read when it arrives.
#[derive(Debug, Clone, Copy)]
pub struct Ask {
    pub answer: KaeriIsFor,
    /// Adult titles show in search and the lists.
    pub adult: bool,
    /// New chapters are notified of.
    pub chapter_notices: bool,
    pub now: i64,
}

impl Ask {
    pub fn of(engine: &EngineState, services: &Services) -> Self {
        let config = engine.config.effective();
        Self {
            answer: engine.reading.answer(),
            adult: config.meta.adult,
            chapter_notices: config.notifications.chapters,
            now: services.clock.now(),
        }
    }

    /// Both media are on: the cross-medium rows and the other scope's count show.
    pub fn both(&self) -> bool {
        self.answer == KaeriIsFor::Both
    }
}

/// Whether the catalogue answers `command`.
pub(crate) fn handles(command: &ReadingCommand) -> bool {
    matches!(
        command,
        ReadingCommand::LoadHome { .. }
            | ReadingCommand::LoadUpdates
            | ReadingCommand::CheckUpdates
            | ReadingCommand::LoadLibrary { .. }
            | ReadingCommand::Search { .. }
            | ReadingCommand::SearchMore { .. }
            | ReadingCommand::RetrySourceSearch { .. }
            | ReadingCommand::LoadSeries { .. }
            | ReadingCommand::LoadVersions { .. }
            | ReadingCommand::HideFromContinue { .. }
            | ReadingCommand::SetStatus { .. }
            | ReadingCommand::SetScore { .. }
    )
}

/// A catalogue command. Refused in place while reading is not available here.
pub(crate) fn command(
    engine: &EngineState,
    services: &Services,
    command: ReadingCommand,
    tx: &Outbox,
) {
    if !engine.reading.available {
        let failure = ReadingFailure::ReadingOff;
        let _ =
            tx.send(Event::Reading(ReadingEvent::Failed { about: command.about(), failure }));
        return;
    }
    let ask = Ask::of(engine, services);
    let kit = &Kit::of(services);
    match command {
        ReadingCommand::LoadHome { refresh } => home::load(kit, ask, refresh, tx),
        ReadingCommand::HideFromContinue { work } => {
            if let Err(e) = kit.store.set_read_hidden(work.as_str(), true) {
                tracing::warn!(error = %e, "the series was not hidden from Continue");
            }
            home::load(kit, ask, false, tx);
        }
        ReadingCommand::LoadUpdates => updates::load(kit, ask, tx),
        ReadingCommand::CheckUpdates => updates::check(kit, ask, tx),
        ReadingCommand::LoadLibrary { segment } => search::library(kit, ask, segment, tx),
        ReadingCommand::Search { query } => search::search(kit, ask, query, 0, tx),
        ReadingCommand::SearchMore { query, page } => {
            search::search(kit, ask, query, page, tx);
        }
        ReadingCommand::RetrySourceSearch { query, source_id } => {
            search::retry_source(kit, ask, query, source_id, tx);
        }
        ReadingCommand::LoadSeries { work } => series::load(kit, ask, work, tx),
        ReadingCommand::LoadVersions { work, chapter } => {
            series::versions(kit, work, chapter, tx);
        }
        ReadingCommand::SetStatus { work, status } => {
            if let Err(e) =
                kit.store.set_read_status(work.as_str(), Some(status.as_str()), ask.now)
            {
                tracing::warn!(error = %e, "the reading list was not kept");
            }
            crate::read_tracking::queue_status(services, &work, status, ask.now, tx);
            series::header(kit, ask, &work, tx);
        }
        ReadingCommand::SetScore { work, score } => {
            let wanted = (score > 0.0).then_some(score);
            let score = wanted.map(f64::from);
            if let Err(e) = kit.store.set_read_score(work.as_str(), score, ask.now) {
                tracing::warn!(error = %e, "the score was not kept");
            }
            crate::read_tracking::queue_score(services, &work, wanted, ask.now, tx);
            series::header(kit, ask, &work, tx);
        }
        other => tracing::debug!(command = ?other, "not a catalogue command"),
    }
}

fn send(tx: &Outbox, event: ReadingEvent) {
    let _ = tx.send(Event::Reading(event));
}

/// Fix this match for a folder's series, from AniList's books.
pub(crate) fn find_match(
    engine: &EngineState,
    services: &Services,
    group: String,
    query: String,
    tx: &Outbox,
) {
    let ask = Ask::of(engine, services);
    search::find_match(&Kit::of(services), ask, group, query, tx);
}
