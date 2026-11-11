//! The reading sources changed: a book plugin installed, updated or removed, a book server or a
//! folder added or removed, a source switched on or off or moved.
//!
//! What was found against the old sources stops holding by itself. A miss, an empty or failed
//! chapter list and a search's answer hold only for the source as it was when it said them, and
//! a series kept as not found only for the sources it was asked of (see
//! [`crate::read_sources::BookSources::source_mark`]). What is on screen is brought
//! up to date, as the shows' titles are: the series pages loaded lately are loaded again with
//! Read from, and Home, the reading list and the search with words are sent again. The series
//! told this session that no source had them, and not on screen, are asked again one at a time
//! behind the launch, so a new source is never asked about the whole library at once.

use std::time::Duration;

use kaeri_core::books::WorkId;
use kaeri_model::reading::LibrarySegmentRead;

use super::{Ask, Kit, editions, home, search};
use crate::outbox::Outbox;

/// How many series pages are loaded again: the ones just looked at. A page opened later asks
/// for itself and finds the sources as they now are.
const RECENT: usize = 3;

/// Between two series asked again behind the screen, so each source is asked gently.
const GAP: Duration = Duration::from_secs(2);

/// What the apps were last sent, to send again when the sources change.
#[derive(Debug, Default)]
pub(crate) struct Shown {
    /// The series whose page was loaded lately, newest last.
    pages: Vec<WorkId>,
    /// The reading list last asked for.
    library: Option<LibrarySegmentRead>,
    /// Home was asked for.
    home: bool,
}

fn shown(kit: &Kit) -> std::sync::MutexGuard<'_, Shown> {
    kit.sources.catalog.shown.lock().unwrap_or_else(|e| e.into_inner())
}

/// `work`'s page was loaded.
pub(crate) fn page(kit: &Kit, work: &WorkId) {
    let mut shown = shown(kit);
    shown.pages.retain(|seen| seen != work);
    shown.pages.push(work.clone());
    if shown.pages.len() > RECENT {
        shown.pages.remove(0);
    }
}

/// A reading list was asked for.
pub(crate) fn library(kit: &Kit, segment: LibrarySegmentRead) {
    shown(kit).library = Some(segment);
}

/// Home was asked for.
pub(crate) fn home(kit: &Kit) {
    shown(kit).home = true;
}

/// The sources changed: bring what is on screen up to date, and ask again behind it about the
/// series told this session that no source had them.
pub(crate) fn sources_changed(kit: &Kit, ask: Ask, tx: &Outbox) {
    kit.sources.adopt_plugins(&kit.plugins);
    let (pages, library, home) = {
        let shown = shown(kit);
        (shown.pages.clone(), shown.library, shown.home)
    };
    let lapsed = lapsed(kit);
    if home {
        home::resend(kit, ask, tx);
    }
    if let Some(segment) = library {
        search::library(kit, ask, segment, tx);
    }
    search::sources_changed(kit, ask, tx);
    for work in &pages {
        editions::again(kit, ask, work.clone(), tx);
    }
    let behind: Vec<WorkId> = lapsed.into_iter().filter(|w| !pages.contains(w)).collect();
    if behind.is_empty() {
        return;
    }
    let (kit, tx) = (kit.clone(), tx.clone());
    tokio::spawn(async move {
        kit.meta.anilist().launch_over().await;
        for work in behind {
            if let Some(check) = editions::check(&kit, ask, &work, &tx) {
                editions::ended(check).await;
            }
            tokio::time::sleep(GAP).await;
        }
    });
}

/// The series kept as not found that the sources as they stand may have, taken out: Home stops
/// saying no source has them. One with a series matched to it since, as a folder's scan
/// matches one, is found; the ones asked of other sources are given, to ask again.
fn lapsed(kit: &Kit) -> Vec<WorkId> {
    let mark = kit.sources.sources_mark();
    let mut kept = kit.sources.catalog.not_found.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = Vec::new();
    kept.retain(|work, (_, asked)| {
        let work = WorkId::new(work.clone());
        if kit.sources.edition_of(&work, None).is_some() {
            return false;
        }
        if *asked != mark {
            out.push(work);
            return false;
        }
        true
    });
    out.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    out
}
