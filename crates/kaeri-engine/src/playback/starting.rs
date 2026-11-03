//! Telling the starting screen what a start is doing: the steps between a Play and the first
//! frame, as [`Event::StartingStep`], and the way to skip the source being asked.

use kaeri_core::ids::AnilistId;
use kaeri_model::{
    Msg,
    event::Event,
    starting::{SourceMiss, StartStep},
};
use kaeri_providers::walk::{self, Miss, Walk};
use tokio::sync::watch;

use crate::outbox::Outbox;

/// Who hears the steps of one play's start, named by the title and episode the screen holds it
/// by, and the skip that reaches the walk through the sources.
#[derive(Clone)]
pub struct Starting {
    tx: Outbox,
    id: AnilistId,
    episode: String,
    skip: Option<watch::Receiver<u64>>,
}

impl Starting {
    pub fn new(
        tx: &Outbox,
        id: AnilistId,
        episode: &str,
        skip: Option<watch::Receiver<u64>>,
    ) -> Self {
        Self { tx: tx.clone(), id, episode: episode.to_owned(), skip }
    }

    /// Say a step, with the step line it brings when it changes it.
    pub fn step(&self, step: StartStep, line: Option<Msg>) {
        say(&self.tx, self.id, &self.episode, step, line);
    }

    /// The walk through the sources, reporting here, with `pinned` the source the show is
    /// pinned to, and skippable when the start can be skipped within.
    pub fn walk(&self, pinned: Option<String>) -> Walk {
        let this = self.clone();
        let walk = Walk::observed(move |step| {
            let (step, line) = told(step);
            this.step(step, line);
        })
        .pinned(pinned);
        match &self.skip {
            Some(skip) => walk.skippable(skip.clone()),
            None => walk,
        }
    }

    /// A watch that only hears a source say it finds peers, for an ask outside the walk.
    pub fn watch(&self) -> Walk {
        self.walk(None)
    }
}

/// What a playback asks how a torrent stream it plays is doing. Answers nothing by default.
#[derive(Clone, Default)]
pub struct Swarm(Option<std::sync::Arc<dyn Fn(&str) -> Option<Pace> + Send + Sync>>);

pub use kaeri_providers::torrent::session::Pace;

impl std::fmt::Debug for Swarm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Swarm")
    }
}

impl Swarm {
    /// A probe that asks the torrent session `torrents` currently holds.
    pub fn of(torrents: kaeri_providers::torrent::bridge::TorrentPlayback) -> Self {
        Self(Some(std::sync::Arc::new(move |url| torrents.session()?.pace_of(url))))
    }

    /// A probe that answers with `pace`, for a test.
    #[cfg(any(test, feature = "testing"))]
    pub fn fixed(pace: Pace) -> Self {
        Self(Some(std::sync::Arc::new(move |_| Some(pace))))
    }

    /// How the stream served at `url` is doing, when it is a torrent's and live.
    pub fn pace(&self, url: &str) -> Option<Pace> {
        self.0.as_ref()?(url)
    }
}

/// Say a step of the start of episode `episode` of title `id` on `tx`.
pub fn say(tx: &Outbox, id: AnilistId, episode: &str, step: StartStep, line: Option<Msg>) {
    let _ = tx.send(Event::StartingStep { id, episode: episode.to_owned(), step, line });
}

/// A step of the walk as the screen is told it.
pub fn told(step: walk::Step) -> (StartStep, Option<Msg>) {
    match step {
        walk::Step::Resting { names } => (StartStep::SourcesResting { names }, None),
        walk::Step::Asking { name, place, total, pinned } => {
            let line = Msg::StartAskingSource { name: name.clone() };
            let step =
                StartStep::AskingSource { name, place: count(place), total: count(total), pinned };
            (step, Some(line))
        }
        walk::Step::Missed { name, miss } => {
            (StartStep::SourceMissed { name, miss: missed(miss) }, None)
        }
        walk::Step::FindingPeers { name } => {
            (StartStep::FindingPeers { name }, Some(Msg::StartFindingPeers))
        }
    }
}

fn missed(miss: Miss) -> SourceMiss {
    match miss {
        Miss::NoMatch => SourceMiss::NoMatch,
        Miss::NoEpisode => SourceMiss::NoEpisode,
        Miss::Failed => SourceMiss::Failed,
        Miss::TimedOut => SourceMiss::TimedOut,
        Miss::Skipped => SourceMiss::Skipped,
    }
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    fn heard() -> (Starting, mpsc::UnboundedReceiver<Event>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Starting::new(&Outbox::from(tx), AnilistId::new(7), "6", None), rx)
    }

    #[tokio::test]
    async fn a_source_asked_is_told_with_its_place_and_the_line_it_brings() {
        let (starting, mut rx) = heard();
        let (step, line) = told(walk::Step::Asking {
            name: "alpha".into(),
            place: 1,
            total: 3,
            pinned: true,
        });
        starting.step(step, line);
        let Some(Event::StartingStep { id, episode, step, line }) = rx.recv().await else {
            panic!("a step is told");
        };
        assert_eq!((id, episode.as_str()), (AnilistId::new(7), "6"));
        assert_eq!(
            step,
            StartStep::AskingSource { name: "alpha".into(), place: 1, total: 3, pinned: true }
        );
        assert_eq!(line, Some(Msg::StartAskingSource { name: "alpha".into() }));
    }

    #[test]
    fn a_source_coming_up_empty_changes_no_line() {
        let (step, line) =
            told(walk::Step::Missed { name: "alpha".into(), miss: Miss::NoEpisode });
        assert_eq!(
            step,
            StartStep::SourceMissed { name: "alpha".into(), miss: SourceMiss::NoEpisode }
        );
        assert_eq!(line, None);
    }
}
