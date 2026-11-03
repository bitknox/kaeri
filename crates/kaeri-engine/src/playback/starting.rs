//! Telling the starting screen what a start is doing: the steps between a Play and the first
//! frame, as [`Event::StartingStep`], and the way to skip the source being asked.

use kaeri_core::ids::AnilistId;
use kaeri_model::{
    Msg,
    event::Event,
    starting::{SourceMiss, StartStep},
};
use kaeri_providers::walk::{self, Miss, Walk};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::{sync::watch, time::Instant};

use crate::outbox::Outbox;

/// Who hears the steps of one play's start, named by the title and episode the screen holds it
/// by, and the skip that reaches the walk through the sources.
#[derive(Clone)]
pub struct Starting {
    tx: Outbox,
    id: AnilistId,
    episode: String,
    skip: Option<watch::Receiver<u64>>,
    /// The sources asked so far, in order, shared by every clone: what a failure names.
    asked: Arc<Mutex<Vec<String>>>,
}

/// How long a start that finds the device offline waits for a connection before it fails.
pub const OFFLINE_WAIT: Duration = Duration::from_secs(120);

impl Starting {
    pub fn new(
        tx: &Outbox,
        id: AnilistId,
        episode: &str,
        skip: Option<watch::Receiver<u64>>,
    ) -> Self {
        Self {
            tx: tx.clone(),
            id,
            episode: episode.to_owned(),
            skip,
            asked: Default::default(),
        }
    }

    /// The names of the sources asked so far, in the order they were asked.
    pub fn asked(&self) -> Vec<String> {
        self.asked.lock().map(|asked| asked.clone()).unwrap_or_default()
    }

    /// Whether any source has been asked yet.
    pub fn has_asked(&self) -> bool {
        self.asked.lock().is_ok_and(|asked| !asked.is_empty())
    }

    /// Wait until `link` (true while offline) says the device is online, until `until`. Says
    /// [`StartStep::WaitingForConnection`] when it has to wait, and is true when it is online,
    /// false at `until`. A dropped link counts as online: nothing is left to wait on.
    pub async fn online(&self, link: &mut watch::Receiver<bool>, until: Instant) -> bool {
        if !*link.borrow_and_update() {
            return true;
        }
        self.step(StartStep::WaitingForConnection, None);
        while *link.borrow_and_update() {
            match tokio::time::timeout_at(until, link.changed()).await {
                Err(_) => return false,
                Ok(Err(_)) => return true,
                Ok(Ok(())) => {}
            }
        }
        true
    }

    /// Run `resolve` once the device is online, waiting up to `wait` for that, and again from
    /// the top when it goes offline before any source has been asked. `None` when the wait ran
    /// out. Dropped, it ends at once, which is how a cancel stops a start that is waiting.
    pub async fn when_connected<T, Fut>(
        &self,
        link: &mut watch::Receiver<bool>,
        wait: Duration,
        mut resolve: impl FnMut() -> Fut,
    ) -> Option<T>
    where
        Fut: std::future::Future<Output = T>,
    {
        let until = Instant::now() + wait;
        loop {
            if !self.online(link, until).await {
                return None;
            }
            tokio::select! {
                biased;
                () = self.offline_before_asking(link) => {}
                resolved = resolve() => return Some(resolved),
            }
        }
    }

    /// Resolves when the device goes offline before any source has been asked: what takes a
    /// start back to waiting for a connection. Pends for good once a source is asked, or when the
    /// link is dropped.
    pub async fn offline_before_asking(&self, link: &mut watch::Receiver<bool>) {
        loop {
            if self.has_asked() {
                return std::future::pending().await;
            }
            if *link.borrow_and_update() {
                return;
            }
            if link.changed().await.is_err() {
                return std::future::pending().await;
            }
        }
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
            if let walk::Step::Asking { name, .. } = &step
                && let Ok(mut asked) = this.asked.lock()
                && !asked.contains(name)
            {
                asked.push(name.clone());
            }
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

type PaceOf = std::sync::Arc<dyn Fn(&str) -> Option<Pace> + Send + Sync>;

/// What a playback asks how a torrent stream it plays is doing. Answers nothing by default.
#[derive(Clone, Default)]
pub struct Swarm(Option<PaceOf>);

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
            let step = StartStep::AskingSource {
                name,
                place: count(place),
                total: count(total),
                pinned,
            };
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

// --- Fixtures ----------------------------------------------------------------------------

/// A made-up start in `state`, as the engine says a real one: for an app's previews and tests,
/// with no network or player. Every event is for episode 7 of The Lamp Road, which
/// resumes at 10:06 unless the state says otherwise, and comes after the staging at 0 ms.
pub fn fixture(
    state: kaeri_model::starting::StartingFixture,
) -> Vec<kaeri_model::starting::FixtureEvent> {
    use kaeri_model::{
        Cause,
        notice::Notice,
        party::NoSourceWhy,
        starting::{FixtureEvent, StartFailure, StartingFixture as State},
    };
    const SHOW: u32 = 900_001;
    const EPISODE: &str = "7";
    const ALPHA: &str = "Alpha";
    const BROOK: &str = "Brook";
    const HARBOR: &str = "Harbor";
    let id = AnilistId::new(SHOW);
    let staged = |resume: Option<f64>| Event::PlaybackStaged {
        id,
        title: "The Lamp Road".into(),
        episode: EPISODE.into(),
        episode_title: Some("The Lamp Seller".into()),
        speed: 1.0,
        resume,
    };
    let step = |step: StartStep, line: Option<Msg>| Event::StartingStep {
        id,
        episode: EPISODE.into(),
        step,
        line,
    };
    let walk = |s: walk::Step| {
        let (s, line) = told(s);
        step(s, line)
    };
    let asking = |name: &str, place, total, pinned| {
        walk(walk::Step::Asking { name: name.into(), place, total, pinned })
    };
    let missed = |name: &str, miss| walk(walk::Step::Missed { name: name.into(), miss });
    let found = |count, source: &str, quality, torrent: bool| {
        let _ = torrent;
        step(
            StartStep::Found {
                count,
                source: Some(source.into()),
                quality: Some(quality),
                translation: kaeri_core::media::Translation::Sub,
            },
            Some(Msg::StartOpeningStream),
        )
    };
    let opening = |attempt, total| step(StartStep::OpeningStream { attempt, total }, None);
    let failed = |why: NoSourceWhy, cause: Cause, asked: &[&str]| {
        let text = crate::playback::resolve::explained(cause);
        let asked = asked.iter().map(|name| (*name).to_owned()).collect();
        [
            Event::Toast(Notice::alert(text.clone())),
            Event::playback_failed(StartFailure { why, text, asked }),
        ]
    };
    let resume = Some(606.0);
    let mut events: Vec<(u64, Event)> = vec![(0, staged(resume))];
    let mut at = |ms: u64, event: Event| events.push((ms, event));
    match state {
        State::TitleLookup => {
            at(0, step(StartStep::LookingUpTitle, Some(Msg::StartLookingUpTitle)))
        }
        State::SourcesLoading => {
            at(0, step(StartStep::LookingUpTitle, Some(Msg::StartLookingUpTitle)));
            at(400, step(StartStep::GettingSourcesReady, Some(Msg::StartGettingSourcesReady)));
        }
        State::AskingFirst => {
            at(0, step(StartStep::LookingUpTitle, Some(Msg::StartLookingUpTitle)));
            at(700, asking(ALPHA, 1, 3, false));
        }
        State::AskingPinned => {
            at(0, step(StartStep::LookingUpTitle, Some(Msg::StartLookingUpTitle)));
            at(700, asking(ALPHA, 1, 3, true));
        }
        State::SourceMissed => {
            at(700, asking(ALPHA, 1, 3, false));
            at(2_600, missed(ALPHA, walk::Miss::NoEpisode));
            at(2_600, asking(BROOK, 2, 3, false));
        }
        State::SourceFailed => {
            at(700, asking(ALPHA, 1, 3, false));
            at(6_000, missed(ALPHA, walk::Miss::TimedOut));
            at(6_000, asking(BROOK, 2, 3, false));
        }
        State::SourcesResting => {
            at(700, walk(walk::Step::Resting { names: vec!["Kumo".into()] }));
            at(700, asking(ALPHA, 1, 2, false));
        }
        State::Slow => {
            at(700, asking(ALPHA, 1, 3, false));
            at(1_500, missed(ALPHA, walk::Miss::NoEpisode));
            at(1_500, asking(BROOK, 2, 3, false));
        }
        State::FindingPeers => {
            at(700, asking(HARBOR, 1, 2, false));
            at(900, walk(walk::Step::FindingPeers { name: Some(HARBOR.into()) }));
        }
        State::PeersKnown => {
            at(700, asking(HARBOR, 1, 2, false));
            at(900, walk(walk::Step::FindingPeers { name: Some(HARBOR.into()) }));
            at(7_000, found(1, HARBOR, 1080, true));
            at(7_100, opening(1, 1));
            at(8_000, step(StartStep::Swarm { peers: 12, bytes_per_second: 0 }, None));
            at(9_000, step(StartStep::Swarm { peers: 12, bytes_per_second: 3_400_000 }, None));
        }
        State::Found => {
            at(700, asking(BROOK, 1, 3, false));
            at(2_300, found(3, BROOK, 1080, false));
            at(2_350, opening(1, 3));
        }
        State::FetchingSubtitles => {
            at(700, asking(BROOK, 1, 3, false));
            at(2_300, found(3, BROOK, 1080, false));
            at(2_350, step(StartStep::FetchingSubtitles, None));
            at(2_400, opening(1, 3));
        }
        State::AnotherStream => {
            at(700, asking(BROOK, 1, 3, false));
            at(2_300, found(3, BROOK, 1080, false));
            at(2_350, opening(1, 3));
            at(
                9_000,
                step(
                    StartStep::TryingAnotherStream { next: 2, total: 3 },
                    Some(Msg::StartTryingAnotherStream),
                ),
            );
            at(9_050, opening(2, 3));
        }
        State::BufferingTorrent => {
            at(700, asking(HARBOR, 1, 2, false));
            at(900, walk(walk::Step::FindingPeers { name: Some(HARBOR.into()) }));
            at(5_000, found(1, HARBOR, 1080, true));
            at(5_050, opening(1, 1));
            at(6_000, step(StartStep::Swarm { peers: 12, bytes_per_second: 3_400_000 }, None));
            for (n, ms) in [(10, 6_100), (25, 7_000), (40, 8_000)] {
                at(ms, step(StartStep::Buffering { percent: n }, Some(Msg::StartBuffering)));
            }
        }
        State::BufferingDirect => {
            at(700, asking(BROOK, 1, 3, false));
            at(2_300, found(3, BROOK, 1080, false));
            at(2_350, opening(1, 3));
            at(3_400, step(StartStep::Speed { bytes_per_second: 1_800_000 }, None));
        }
        // Offline, the start waits and says so; the connection is back at 8 s and the walk
        // begins by itself.
        State::Offline => {
            at(0, Event::Connectivity { offline: true });
            at(0, step(StartStep::WaitingForConnection, None));
            at(8_000, Event::Connectivity { offline: false });
            at(8_700, asking(ALPHA, 1, 2, false));
        }
        // Still offline after the two minutes: nothing was asked and the start fails.
        State::OfflineTimedOut => {
            at(0, Event::Connectivity { offline: true });
            at(0, step(StartStep::WaitingForConnection, None));
            for event in failed(NoSourceWhy::NoConnection, Cause::NoConnection, &[]) {
                at(120_000, event);
            }
        }
        State::FailedNoEpisode => {
            at(700, asking(ALPHA, 1, 2, false));
            at(1_500, missed(ALPHA, walk::Miss::NoEpisode));
            at(1_500, asking(BROOK, 2, 2, false));
            at(2_300, missed(BROOK, walk::Miss::NoEpisode));
            for event in failed(
                NoSourceWhy::NotFound,
                Cause::NoSourceHasEpisode {
                    episode: EPISODE.into(),
                    detail: "alpha: not found, brook: not found".into(),
                },
                &[ALPHA, BROOK],
            ) {
                at(2_400, event);
            }
        }
        State::FailedNoMatch => {
            at(700, asking(ALPHA, 1, 1, false));
            at(1_500, missed(ALPHA, walk::Miss::NoMatch));
            for event in failed(
                NoSourceWhy::NotFound,
                Cause::TitleNoMatch { detail: "alpha: no match".into() },
                &[ALPHA],
            ) {
                at(1_600, event);
            }
        }
        State::FailedNoSources => {
            for event in failed(NoSourceWhy::NoSources, Cause::NoSources, &[]) {
                at(100, event);
            }
        }
        State::FailedNotAired => {
            at(0, step(StartStep::LookingUpTitle, Some(Msg::StartLookingUpTitle)));
            let text = Msg::NoticeNotAired {
                episode: 7,
                when: Cause::AirsTomorrow { time: "18:30".into() },
            };
            at(600, Event::Toast(Notice::alert(text.clone())));
            at(
                600,
                Event::playback_failed(StartFailure {
                    why: NoSourceWhy::NotAired,
                    text,
                    asked: Vec::new(),
                }),
            );
        }
    }
    // A slow start carries on to 24 seconds with nothing new said.
    if state == State::Slow {
        at(24_000, Event::Connectivity { offline: false });
    }
    // Only the Offline and failed states resume at nothing: the rest resume at 10:06.
    events.into_iter().map(|(at_ms, event)| FixtureEvent { at_ms, event }).collect()
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
        let (step, line) =
            told(walk::Step::Asking { name: "alpha".into(), place: 1, total: 3, pinned: true });
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

    #[test]
    fn every_fixture_stages_first_and_stays_in_order() {
        use kaeri_model::starting::StartingFixture as F;
        for state in [
            F::TitleLookup,
            F::SourcesLoading,
            F::AskingFirst,
            F::AskingPinned,
            F::SourceMissed,
            F::SourceFailed,
            F::SourcesResting,
            F::Slow,
            F::FindingPeers,
            F::PeersKnown,
            F::Found,
            F::FetchingSubtitles,
            F::AnotherStream,
            F::BufferingTorrent,
            F::BufferingDirect,
            F::Offline,
            F::OfflineTimedOut,
            F::FailedNoEpisode,
            F::FailedNoMatch,
            F::FailedNoSources,
            F::FailedNotAired,
        ] {
            let events = fixture(state);
            assert!(matches!(events[0].event, Event::PlaybackStaged { .. }), "{state:?}");
            assert!(events.windows(2).all(|w| w[0].at_ms <= w[1].at_ms), "{state:?}");
        }
        let failed = fixture(F::FailedNoSources);
        assert!(
            failed
                .iter()
                .any(|e| matches!(&e.event, Event::PlaybackEnded { failure: Some(_), .. }))
        );
    }

    #[tokio::test(start_paused = true)]
    async fn an_online_start_asks_at_once_and_says_nothing_of_waiting() {
        let (starting, mut rx) = heard();
        let (_tx, mut link) = watch::channel(false);
        let got = starting.when_connected(&mut link, OFFLINE_WAIT, || async { 7 }).await;
        assert_eq!(got, Some(7));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test(start_paused = true)]
    async fn an_offline_start_waits_says_so_once_and_goes_on_when_back() {
        let (starting, mut rx) = heard();
        let (tx, mut link) = watch::channel(true);
        let started = Instant::now();
        let back = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(30)).await;
            tx.send_replace(false);
            tx
        });
        let got = starting.when_connected(&mut link, OFFLINE_WAIT, || async { "walked" }).await;
        assert_eq!(got, Some("walked"));
        assert_eq!(started.elapsed(), Duration::from_secs(30));
        let Some(Event::StartingStep { step, .. }) = rx.recv().await else { panic!("a step") };
        assert_eq!(step, StartStep::WaitingForConnection);
        assert!(rx.try_recv().is_err(), "said once");
        back.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn a_start_offline_for_two_minutes_gives_up() {
        let (starting, _rx) = heard();
        let (_tx, mut link) = watch::channel(true);
        let started = Instant::now();
        let got = starting.when_connected(&mut link, OFFLINE_WAIT, || async { 1 }).await;
        assert_eq!(got, None);
        assert_eq!(started.elapsed(), OFFLINE_WAIT);
    }

    #[tokio::test(start_paused = true)]
    async fn a_start_that_goes_offline_before_asking_waits_and_begins_again() {
        let (starting, mut rx) = heard();
        let (tx, mut link) = watch::channel(false);
        let runs = std::sync::atomic::AtomicU32::new(0);
        let dropped = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            tx.send_replace(true);
            tokio::time::sleep(Duration::from_secs(10)).await;
            tx.send_replace(false);
            tx
        });
        let got = starting
            .when_connected(&mut link, OFFLINE_WAIT, || {
                let run = runs.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                async move { if run == 0 { std::future::pending::<u32>().await } else { run } }
            })
            .await;
        assert_eq!(got, Some(1), "the walk began again on reconnect");
        let Some(Event::StartingStep { step, .. }) = rx.recv().await else { panic!("a step") };
        assert_eq!(step, StartStep::WaitingForConnection);
        dropped.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn a_start_that_has_asked_a_source_carries_on_offline() {
        let (starting, _rx) = heard();
        let (tx, mut link) = watch::channel(false);
        let (step, line) = told(walk::Step::Asking {
            name: "alpha".into(),
            place: 1,
            total: 1,
            pinned: false,
        });
        starting.step(step, line);
        starting.asked.lock().unwrap().push("alpha".into());
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            tx.send_replace(true);
            std::future::pending::<()>().await
        });
        let got = starting
            .when_connected(&mut link, OFFLINE_WAIT, || async {
                tokio::time::sleep(Duration::from_secs(5)).await;
                "done"
            })
            .await;
        assert_eq!(got, Some("done"));
        assert_eq!(starting.asked(), ["alpha"]);
    }

    #[test]
    fn the_offline_fixtures_wait_then_start_or_fail_after_two_minutes() {
        use kaeri_model::starting::StartingFixture as F;
        let waits = fixture(F::Offline);
        assert!(waits.iter().any(|e| matches!(
            &e.event,
            Event::StartingStep { step: StartStep::WaitingForConnection, .. }
        )));
        assert!(waits.iter().any(|e| matches!(
            &e.event,
            Event::StartingStep { step: StartStep::AskingSource { .. }, .. }
        )));
        let failed = fixture(F::OfflineTimedOut);
        let last = failed.last().unwrap();
        assert_eq!(last.at_ms, 120_000);
        assert!(matches!(
            &last.event,
            Event::PlaybackEnded { failure: Some(f), .. }
                if f.why == kaeri_model::party::NoSourceWhy::NoConnection
        ));
        let asked = fixture(F::FailedNoEpisode);
        assert!(asked.iter().any(|e| matches!(
            &e.event,
            Event::PlaybackEnded { failure: Some(f), .. } if f.asked == ["Alpha", "Brook"]
        )));
    }
}
