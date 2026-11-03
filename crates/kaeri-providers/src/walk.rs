//! Watching a walk through the sources: which one is asked, one coming up empty, a torrent
//! finding peers, and a way to skip the one being asked.
//!
//! [`first_answer_with`](crate::resolve::first_answer_with) reports each step to a [`Walk`]'s
//! observer. This crate has no outbox, so the engine turns the steps into events. A source
//! whose answer waits on something slow, as a torrent waits for peers, says so from where it
//! waits with [`finding_peers`], which reaches the walk that is asking it without the source
//! knowing of it.

use std::sync::Arc;

use tokio::sync::watch;

/// Why a source came up empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Miss {
    /// The source has no such title.
    NoMatch,
    /// The source has the title and not this episode.
    NoEpisode,
    /// The source answered with an error.
    Failed,
    /// The source did not answer in time.
    TimedOut,
    /// The viewer skipped it while it was asked.
    Skipped,
}

/// One step of a walk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Sources passed over because they are resting after failures. Said once, first.
    Resting { names: Vec<String> },
    /// One source is asked. `place` counts from 1 among `total`.
    Asking { name: String, place: usize, total: usize, pinned: bool },
    /// A source came up empty.
    Missed { name: String, miss: Miss },
    /// A torrent is waiting for peers to send its metadata, for the source being asked, or for
    /// a link the viewer opened, which has none.
    FindingPeers { name: Option<String> },
}

type Observer = Arc<dyn Fn(Step) + Send + Sync>;

/// What watches a walk, and how to skip within it. The default watches nothing.
#[derive(Clone, Default)]
pub struct Walk {
    pub(crate) observer: Option<Observer>,
    pub(crate) skip: Option<watch::Receiver<u64>>,
    pub(crate) pinned: Option<String>,
}

impl Walk {
    /// A walk whose steps are given to `observer`.
    pub fn observed(observer: impl Fn(Step) + Send + Sync + 'static) -> Self {
        Self { observer: Some(Arc::new(observer)), ..Self::default() }
    }

    /// The walk ends the source being asked when the counter `skip` watches moves.
    pub fn skippable(mut self, skip: watch::Receiver<u64>) -> Self {
        self.skip = Some(skip);
        self
    }

    /// The source id the show is pinned to, which is flagged as such when it is asked.
    pub fn pinned(mut self, provider_id: Option<String>) -> Self {
        self.pinned = provider_id;
        self
    }

    pub(crate) fn say(&self, step: Step) {
        if let Some(observer) = &self.observer {
            observer(step);
        }
    }
}

impl std::fmt::Debug for Walk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Walk")
            .field("observed", &self.observer.is_some())
            .field("skippable", &self.skip.is_some())
            .field("pinned", &self.pinned)
            .finish()
    }
}

/// The source being asked and who watches it, for the task that asks.
#[derive(Clone)]
struct Asked {
    name: Option<String>,
    observer: Observer,
}

tokio::task_local! {
    static ASKED: Asked;
}

/// Run `asking` with `name` as the source asked, if any, and `walk` watching, so that a step the source
/// says from inside reaches the observer.
pub async fn asking<T>(
    walk: &Walk,
    name: Option<&str>,
    asking: impl std::future::Future<Output = T>,
) -> T {
    match &walk.observer {
        Some(observer) => {
            ASKED
                .scope(
                    Asked { name: name.map(str::to_owned), observer: Arc::clone(observer) },
                    asking,
                )
                .await
        }
        None => asking.await,
    }
}

/// Say, from inside a source's answer, that a torrent is finding peers. Nothing happens when
/// no walk watches the one asking.
pub fn finding_peers() {
    let _ = ASKED.try_with(|asked| {
        (asked.observer)(Step::FindingPeers { name: asked.name.clone() });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[tokio::test]
    async fn a_source_says_it_is_finding_peers_to_the_walk_that_asks_it() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        let walk = Walk::observed(move |step| log.lock().unwrap().push(step));
        finding_peers();
        assert!(seen.lock().unwrap().is_empty(), "no walk asks: nothing is said");
        asking(&walk, Some("alpha"), async { finding_peers() }).await;
        assert_eq!(
            *seen.lock().unwrap(),
            vec![Step::FindingPeers { name: Some("alpha".into()) }]
        );
    }
}
