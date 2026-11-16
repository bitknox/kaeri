//! The sources a playback walks on to once the viewer has moved on from every stream it found:
//! a skip of the source being opened, or Switch source on a stall, with no other stream left in
//! the pool. The walk through the sources stops at the first source that answers, so a pool is
//! often that one source's streams, and the sources after it are where the next stream is.

use std::sync::Arc;

use kaeri_core::stream::Stream;
use kaeri_model::starting::StartFailure;
use tokio::sync::watch;

/// Where a playback finds streams beyond the ones it was given, and the skip that asks for
/// them. The default has neither: a playback that cannot look further.
#[derive(Clone, Default)]
pub struct Further {
    /// The count the starting screen's Skip moves, as the walk through the sources hears it,
    /// marked as seen when the walk handed its streams over.
    skips: Option<watch::Receiver<u64>>,
    lookup: Option<Arc<dyn Lookup>>,
}

impl std::fmt::Debug for Further {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Further")
            .field("skips", &self.skips.is_some())
            .field("lookup", &self.lookup.is_some())
            .finish()
    }
}

/// Asks the sources a playback has not had streams from yet.
#[async_trait::async_trait]
pub trait Lookup: Send + Sync {
    /// Whether any source is left to ask once the sources of `streams`, and those already
    /// asked, are passed over.
    fn beyond(&self, streams: &[Stream]) -> bool;

    /// The streams of the next source that answers, passing over the sources of `streams` and
    /// those already asked, with the steps said on the starting screen as the first walk said
    /// them. `skipped` is the source the viewer skipped, said as skipped before the walk goes on.
    /// The failure the start would have ended in when none answers.
    async fn after(
        &self,
        streams: &[Stream],
        skipped: Option<&str>,
    ) -> Result<Vec<Stream>, StartFailure>;
}

impl Further {
    /// A playback that hears `skips` and finds more streams through `lookup`. The skips made
    /// so far were the walk's, and are marked as seen.
    pub fn new(mut skips: watch::Receiver<u64>, lookup: Arc<dyn Lookup>) -> Self {
        skips.mark_unchanged();
        Self { skips: Some(skips), lookup: Some(lookup) }
    }

    /// A playback that hears `skips` and has nowhere further to look: a link the viewer opened,
    /// or a release they picked.
    pub fn skips_only(mut skips: watch::Receiver<u64>) -> Self {
        skips.mark_unchanged();
        Self { skips: Some(skips), lookup: None }
    }

    /// The skips the playback hears, from here on.
    pub(crate) fn skips(&self) -> Option<watch::Receiver<u64>> {
        self.skips.clone()
    }

    /// Whether a source is left to ask beyond those `streams` came from.
    pub(crate) fn beyond(&self, streams: &[Stream]) -> bool {
        self.lookup.as_ref().is_some_and(|lookup| lookup.beyond(streams))
    }

    /// See [`Lookup::after`]. Nothing to ask is a failure with nobody asked.
    #[allow(clippy::result_large_err)]
    pub(crate) async fn after(
        &self,
        streams: &[Stream],
        skipped: Option<&str>,
    ) -> Result<Vec<Stream>, StartFailure> {
        match &self.lookup {
            Some(lookup) => lookup.after(streams, skipped).await,
            None => Err(StartFailure {
                why: kaeri_model::party::NoSourceWhy::NotFound,
                text: kaeri_model::Msg::NoticeReason { reason: kaeri_model::Cause::NoSources },
                asked: Vec::new(),
                unreached: Vec::new(),
                held_back: Vec::new(),
            }),
        }
    }
}
