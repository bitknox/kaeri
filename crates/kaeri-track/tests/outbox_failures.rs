//! What the outboxes do when a push fails, against a fake tracker: a change the tracker refuses,
//! days without a network, a tracker that is down, a rejected credential, a pause it asks for.
//! Every case runs for both queues (watching and reading) and both kinds of tracker: one that
//! writes a batch in one request and can only fail it whole, and one that writes title by title
//! and answers for each.

use std::sync::Mutex;

use kaeri_core::{
    Error,
    ids::AnilistId,
    traits::{PushFailure, Pushed, TrackOp, TrackedEntry, Tracker, WatchStatus},
};
use kaeri_store::Store;
use kaeri_track::sync::{self, DrainReport};

const ID: &str = "fake";
const DAY: i64 = 86_400;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mode {
    Up,
    /// No network: the request never leaves.
    Offline,
    /// The tracker answers every request with a `5xx`.
    Down,
    /// The tracker rejects the token.
    SignedOut,
    /// The tracker asks for a pause of this many seconds.
    Paused(u64),
}

#[derive(Default)]
struct Seen {
    pushes: u32,
    sent: Vec<TrackOp>,
}

struct Fake {
    batched: bool,
    mode: Mutex<Mode>,
    /// Titles the tracker refuses for good, as for an entry it removed.
    refused: Mutex<Vec<u32>>,
    /// Titles the tracker fails on its own side while answering for the rest.
    troubled: Mutex<Vec<u32>>,
    seen: Mutex<Seen>,
}

impl Fake {
    fn new(batched: bool) -> Self {
        Self {
            batched,
            mode: Mutex::new(Mode::Up),
            refused: Mutex::default(),
            troubled: Mutex::default(),
            seen: Mutex::default(),
        }
    }

    fn set(&self, mode: Mode) {
        *self.mode.lock().unwrap() = mode;
    }

    fn refuse(&self, id: u32) {
        self.refused.lock().unwrap().push(id);
    }

    fn trouble(&self, ids: &[u32]) {
        *self.troubled.lock().unwrap() = ids.to_vec();
    }

    fn pushes(&self) -> u32 {
        self.seen.lock().unwrap().pushes
    }

    fn sent(&self) -> Vec<TrackOp> {
        self.seen.lock().unwrap().sent.clone()
    }

    fn sent_for(&self, id: u32) -> Vec<TrackOp> {
        self.sent().into_iter().filter(|op| op.anilist_id().get() == id).collect()
    }

    fn outage(&self) -> Option<Error> {
        let tracker = ID.to_owned();
        match *self.mode.lock().unwrap() {
            Mode::Up => None,
            Mode::Offline => Some(Error::Network("dns error".into())),
            Mode::Down => Some(Error::Unavailable { tracker, message: "503".into() }),
            Mode::SignedOut => Some(Error::Auth("the token was rejected".into())),
            Mode::Paused(secs) => Some(Error::RateLimited {
                tracker,
                message: "429".into(),
                retry_in_secs: Some(secs),
            }),
        }
    }

    fn answer(&self, ops: &[TrackOp]) -> Result<Pushed, Error> {
        let mut seen = self.seen.lock().unwrap();
        seen.pushes += 1;
        let refused = self.refused.lock().unwrap().clone();
        let troubled = self.troubled.lock().unwrap().clone();
        let refusal =
            |message: &str| Error::Tracker { tracker: ID.into(), message: message.into() };
        if self.batched {
            // One request for the batch: any failure fails all of it.
            if let Some(error) = self.outage() {
                return Err(error);
            }
            if ops.iter().any(|op| refused.contains(&op.anilist_id().get())) {
                return Err(refusal("Not Found."));
            }
            if ops.iter().any(|op| troubled.contains(&op.anilist_id().get())) {
                return Err(Error::Unavailable { tracker: ID.into(), message: "500".into() });
            }
            seen.sent.extend(ops.iter().cloned());
            return Ok(Pushed::default());
        }
        // Title by title, as the trackers that write each title on its own do.
        let mut titles: Vec<AnilistId> = Vec::new();
        for op in ops {
            if !titles.contains(&op.anilist_id()) {
                titles.push(op.anilist_id());
            }
        }
        let mut pushed = Pushed::default();
        for title in titles {
            let failure = match self.outage() {
                Some(error) => Some(PushFailure::of(&error)),
                None if refused.contains(&title.get()) => {
                    Some(PushFailure::Refused("Not Found.".into()))
                }
                None if troubled.contains(&title.get()) => {
                    Some(PushFailure::Unavailable("500".into()))
                }
                None => None,
            };
            match failure {
                Some(failure) => pushed.failed.push((title, failure)),
                None => {
                    seen.sent.extend(ops.iter().filter(|op| op.anilist_id() == title).cloned())
                }
            }
        }
        Ok(pushed)
    }
}

#[async_trait::async_trait]
impl Tracker for Fake {
    fn id(&self) -> &str {
        ID
    }
    fn is_authenticated(&self) -> bool {
        true
    }
    fn serves_reading(&self) -> bool {
        true
    }
    async fn pull_library(&self) -> Result<Vec<TrackedEntry>, Error> {
        Ok(Vec::new())
    }
    async fn push(&self, ops: &[TrackOp]) -> Result<(), Error> {
        self.answer(ops)?.into_result(ID)
    }
    async fn push_checked(&self, ops: &[TrackOp]) -> Result<Pushed, Error> {
        self.answer(ops)
    }
    async fn push_read(&self, ops: &[TrackOp]) -> Result<Pushed, Error> {
        self.answer(ops)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Media {
    Watching,
    Reading,
}

const BOTH: [Media; 2] = [Media::Watching, Media::Reading];

impl Media {
    fn status(self, id: u32, status: WatchStatus, at: i64) -> TrackOp {
        let anilist_id = AnilistId::new(id);
        match self {
            Self::Watching => TrackOp::SetStatus { anilist_id, status, at },
            Self::Reading => {
                TrackOp::SetReadStatus { anilist_id, status, rereading: false, at }
            }
        }
    }

    fn progress(self, id: u32, n: u32) -> TrackOp {
        let anilist_id = AnilistId::new(id);
        match self {
            Self::Watching => TrackOp::SetProgress { anilist_id, episode: n },
            Self::Reading => TrackOp::SetReadProgress { anilist_id, chapter: n, volume: None },
        }
    }

    fn score(self, id: u32, score: f32, at: i64) -> TrackOp {
        let anilist_id = AnilistId::new(id);
        match self {
            Self::Watching => TrackOp::SetScore { anilist_id, score, at },
            Self::Reading => TrackOp::SetReadScore { anilist_id, score, at },
        }
    }

    /// Three changes to three titles.
    fn three(self) -> Vec<TrackOp> {
        vec![
            self.progress(990_001, 42),
            self.status(990_002, WatchStatus::Completed, 1_000),
            self.score(990_003, 8.5, 1_000),
        ]
    }

    async fn drain(self, store: &Store, tracker: &Fake, now: i64) -> DrainReport {
        match self {
            Self::Watching => sync::drain(store, tracker, now).await.unwrap(),
            Self::Reading => sync::drain_read(store, tracker, now).await.unwrap(),
        }
    }

    fn depth(self, store: &Store) -> u32 {
        match self {
            Self::Watching => store.outbox_depth(Some(ID)).unwrap(),
            Self::Reading => store.read_outbox_depth(Some(ID)).unwrap(),
        }
    }

    /// Refusals counted against each change still queued, whenever it is due.
    fn attempts(self, store: &Store) -> Vec<u32> {
        let all = match self {
            Self::Watching => store.claim_ready(ID, i64::MAX / 2, 1_000),
            Self::Reading => store.claim_ready_read(ID, i64::MAX / 2, 1_000),
        };
        all.unwrap().iter().map(|entry| entry.attempts).collect()
    }
}

/// Every case, for each queue and each kind of tracker.
fn cases() -> Vec<(Media, bool)> {
    BOTH.into_iter().flat_map(|media| [(media, true), (media, false)]).collect()
}

/// Drain every minute from `from` until `until`, adding up what was sent and given up.
async fn tick(
    media: Media,
    store: &Store,
    tracker: &Fake,
    from: i64,
    until: i64,
) -> (u32, u32) {
    let (mut sent, mut dropped) = (0, 0);
    let mut now = from;
    while now < until {
        let report = media.drain(store, tracker, now).await;
        sent += report.sent;
        dropped += report.dropped;
        now += 60;
    }
    (sent, dropped)
}

/// A change the tracker refuses for good, queued first, with good changes behind it in the same
/// batch: the good ones go at once, and the refused one is given up after its refusals and kept
/// for the app to show, without a request a minute in between.
#[tokio::test]
async fn one_refused_change_does_not_sink_the_changes_queued_behind_it() {
    for (media, batched) in cases() {
        let store = Store::open_in_memory().unwrap();
        store.enqueue(ID, &media.progress(990_099, 3), 900).unwrap();
        for op in media.three() {
            store.enqueue(ID, &op, 1_000).unwrap();
        }
        let tracker = Fake::new(batched);
        tracker.refuse(990_099);

        let first = media.drain(&store, &tracker, 1_000).await;
        assert_eq!(first.sent, 3, "{media:?} batched {batched}: the good changes go at once");
        assert_eq!(media.depth(&store), 1, "{media:?} batched {batched}");

        let (_, dropped) = tick(media, &store, &tracker, 1_060, 1_000 + 3 * DAY).await;
        assert_eq!(dropped, 1, "{media:?} batched {batched}: the refused change is given up");
        assert_eq!(media.depth(&store), 0, "{media:?} batched {batched}");
        let kept = store.dropped_track_ops().unwrap();
        assert_eq!(kept.len(), 1, "{media:?} batched {batched}");
        assert_eq!(kept[0].op, media.progress(990_099, 3));
        assert_eq!(kept[0].tracker_id, ID);
        assert!(kept[0].error.contains("Not Found."), "{:?}", kept[0].error);
        // Fifty refusals on a backoff, plus the isolation requests of the first pass.
        assert!(
            tracker.pushes() < 60,
            "{media:?} batched {batched}: {} pushes",
            tracker.pushes()
        );
    }
}

/// The app left open without a network for three days, the drain ticking each minute: nothing
/// queued is lost or counted, and all of it goes when the network is back. The same when the
/// network works and the tracker answers every request with a `5xx`.
#[tokio::test]
async fn three_days_without_the_tracker_lose_nothing() {
    for outage in [Mode::Offline, Mode::Down] {
        for (media, batched) in cases() {
            let case = format!("{outage:?} {media:?} batched {batched}");
            let store = Store::open_in_memory().unwrap();
            for op in media.three() {
                store.enqueue(ID, &op, 1_000).unwrap();
            }
            let tracker = Fake::new(batched);
            tracker.set(outage);

            let end = 1_000 + 3 * DAY;
            let (sent, dropped) = tick(media, &store, &tracker, 1_000, end).await;
            assert_eq!((sent, dropped), (0, 0), "{case}");
            assert_eq!(media.depth(&store), 3, "{case}: every change is still queued");
            assert_eq!(media.attempts(&store), [0, 0, 0], "{case}: nothing was counted");
            assert!(store.dropped_track_ops().unwrap().is_empty(), "{case}");
            // A backoff up to an hour while it stays down, rather than a request each minute. A
            // batch failed by the tracker's own trouble is asked again title by title, which
            // stops at the second failure: three requests a pass at most.
            let passes = tracker.pushes() / if batched && outage == Mode::Down { 3 } else { 1 };
            assert!(passes < 100, "{case}: {} pushes", tracker.pushes());

            tracker.set(Mode::Up);
            let (sent, _) = tick(media, &store, &tracker, end, end + 3_600).await;
            assert_eq!(sent, 3, "{case}: every change made offline reaches the tracker");
            assert_eq!(tracker.sent(), media.three(), "{case}: in the order they were made");
            assert_eq!(media.depth(&store), 0, "{case}");
        }
    }
}

/// A tracker that went down holds both queues: a reading drain right after a failed watching
/// drain does not ask it again before the hold is over.
#[tokio::test]
async fn an_outage_seen_by_one_queue_holds_the_other() {
    let store = Store::open_in_memory().unwrap();
    for op in Media::Watching.three().into_iter().chain(Media::Reading.three()) {
        store.enqueue(ID, &op, 1_000).unwrap();
    }
    let tracker = Fake::new(false);
    tracker.set(Mode::Offline);
    Media::Watching.drain(&store, &tracker, 1_000).await;
    let asked = tracker.pushes();
    let report = Media::Reading.drain(&store, &tracker, 1_001).await;
    assert_eq!(tracker.pushes(), asked, "the reading queue waits out the same hold");
    assert_eq!(report.remaining, 3);
    assert!(report.did_nothing());
}

/// A rejected credential holds everything, however long it lasts: nothing is counted or given
/// up, and a sign-in sends all of it.
#[tokio::test]
async fn an_expired_sign_in_holds_every_change_until_the_next_sign_in() {
    for (media, batched) in cases() {
        let case = format!("{media:?} batched {batched}");
        let store = Store::open_in_memory().unwrap();
        for op in media.three() {
            store.enqueue(ID, &op, 1_000).unwrap();
        }
        let tracker = Fake::new(batched);
        tracker.set(Mode::SignedOut);

        let first = media.drain(&store, &tracker, 1_000).await;
        assert!(first.needs_reauth, "{case}");
        let end = 1_000 + 7 * DAY;
        tick(media, &store, &tracker, 1_060, end).await;
        assert_eq!(media.depth(&store), 3, "{case}");
        assert_eq!(media.attempts(&store), [0, 0, 0], "{case}");
        assert!(store.dropped_track_ops().unwrap().is_empty(), "{case}");
        // The hold asks once an hour, not once a minute.
        assert!(tracker.pushes() <= 7 * 24 + 1, "{case}: {} pushes", tracker.pushes());

        tracker.set(Mode::Up);
        sync::clear_reauth_hold(&store, ID).unwrap();
        let report = media.drain(&store, &tracker, end).await;
        assert_eq!(report.sent, 3, "{case}: the sign-in sends everything held");
        assert!(!report.needs_reauth, "{case}");
    }
}

/// A pause the tracker asks for is waited out and not counted.
#[tokio::test]
async fn a_pause_the_tracker_asks_for_is_waited_out_and_not_counted() {
    for (media, batched) in cases() {
        let case = format!("{media:?} batched {batched}");
        let store = Store::open_in_memory().unwrap();
        for op in media.three() {
            store.enqueue(ID, &op, 1_000).unwrap();
        }
        let tracker = Fake::new(batched);
        tracker.set(Mode::Paused(600));

        let report = media.drain(&store, &tracker, 1_000).await;
        assert_eq!((report.sent, report.dropped, report.remaining), (0, 0, 3), "{case}");
        tracker.set(Mode::Up);
        let early = media.drain(&store, &tracker, 1_300).await;
        assert!(early.did_nothing(), "{case}: the pause it asked for is kept");
        assert_eq!(media.attempts(&store), [0, 0, 0], "{case}");
        let after = media.drain(&store, &tracker, 1_600).await;
        assert_eq!(after.sent, 3, "{case}");
    }
}

/// The tracker failing on its own side for one title, while it answers for the others: that
/// title waits, uncounted, and the rest of the queue keeps moving. A tracker that writes the
/// batch in one request fails all of it, and is asked again title by title to find which.
#[tokio::test]
async fn trouble_with_one_title_does_not_hold_up_the_others() {
    for (media, batched) in cases() {
        let case = format!("{media:?} batched {batched}");
        let store = Store::open_in_memory().unwrap();
        for op in media.three() {
            store.enqueue(ID, &op, 1_000).unwrap();
        }
        let tracker = Fake::new(batched);
        tracker.trouble(&[990_001]);

        let first = media.drain(&store, &tracker, 1_000).await;
        assert_eq!((first.sent, first.remaining), (2, 1), "{case}");
        assert_eq!(media.attempts(&store), [0], "{case}: not counted");

        // A change made a minute later goes on the next pass: the tracker is not on hold.
        store.enqueue(ID, &media.progress(990_004, 1), 1_050).unwrap();
        let next = media.drain(&store, &tracker, 1_060).await;
        assert_eq!(next.sent, 1, "{case}");

        tracker.trouble(&[]);
        let (sent, _) = tick(media, &store, &tracker, 1_120, 1_000 + 2 * 3_600).await;
        assert_eq!(sent, 1, "{case}: the troubled title goes once the trouble is over");
        assert_eq!(media.depth(&store), 0, "{case}");
    }
}

/// A title's changes go in the order they were made: a newer change never overtakes an older
/// one that is waiting, and once the older one may go they go together.
#[tokio::test]
async fn a_newer_change_never_overtakes_an_older_one_for_the_same_title() {
    for (media, batched) in cases() {
        let case = format!("{media:?} batched {batched}");
        let store = Store::open_in_memory().unwrap();
        store.enqueue(ID, &media.status(990_001, WatchStatus::Current, 1_000), 1_000).unwrap();
        let tracker = Fake::new(batched);
        tracker.refuse(990_001);
        let first = media.drain(&store, &tracker, 1_000).await;
        assert_eq!(first.sent, 0, "{case}");

        // The newer status, queued while the older one waits out its backoff.
        store
            .enqueue(ID, &media.status(990_001, WatchStatus::Completed, 1_010), 1_010)
            .unwrap();
        tracker.refused.lock().unwrap().clear();
        let early = media.drain(&store, &tracker, 1_015).await;
        assert_eq!(early.sent, 0, "{case}: the newer status waits behind the older one");
        assert!(tracker.sent().is_empty(), "{case}");

        let later = media.drain(&store, &tracker, 1_000 + 3_600).await;
        assert_eq!(later.sent, 2, "{case}: both go together");
        assert_eq!(
            tracker.sent_for(990_001),
            [
                media.status(990_001, WatchStatus::Current, 1_000),
                media.status(990_001, WatchStatus::Completed, 1_010),
            ],
            "{case}: older first"
        );
    }
}

/// Progress made while the tracker is away still collapses into one change, and the furthest
/// point goes when it is back.
#[tokio::test]
async fn progress_made_during_an_outage_still_collapses() {
    for (media, batched) in cases() {
        let case = format!("{media:?} batched {batched}");
        let store = Store::open_in_memory().unwrap();
        let tracker = Fake::new(batched);
        tracker.set(Mode::Offline);
        store.enqueue(ID, &media.progress(990_001, 3), 1_000).unwrap();
        media.drain(&store, &tracker, 1_000).await;
        for n in [4, 6, 5] {
            store.enqueue(ID, &media.progress(990_001, n), 1_100).unwrap();
        }
        assert_eq!(media.depth(&store), 1, "{case}");

        tracker.set(Mode::Up);
        tick(media, &store, &tracker, 1_200, 1_000 + 3_600).await;
        assert_eq!(tracker.sent(), [media.progress(990_001, 6)], "{case}");
    }
}

/// What became of a change while the clock ran ahead.
#[derive(Debug, Clone, Copy)]
enum Ahead {
    /// Only queued.
    Queued,
    /// The tracker refused it, that once.
    Refused,
    /// Its push failed this way.
    Failed(Mode),
}

/// A change whose push failed while the computer's clock ran a year ahead (a dead clock
/// battery, a wrong manual date), then the clock put right: it goes within the hour, whatever
/// the failure was, rather than wait for the clock to catch up with the wait it was given. The
/// same for a change only queued while the clock was ahead.
#[tokio::test]
async fn a_change_that_failed_while_the_clock_ran_ahead_still_goes_out() {
    let ahead = 1_000 + 365 * DAY;
    let fates = [
        Ahead::Queued,
        Ahead::Refused,
        Ahead::Failed(Mode::Offline),
        Ahead::Failed(Mode::Down),
        Ahead::Failed(Mode::Paused(600)),
        Ahead::Failed(Mode::SignedOut),
    ];
    for (media, batched) in cases() {
        for fate in fates {
            let case = format!("{fate:?} {media:?} batched {batched}");
            let store = Store::open_in_memory().unwrap();
            store.enqueue(ID, &media.progress(990_001, 42), ahead).unwrap();
            let tracker = Fake::new(batched);
            match fate {
                Ahead::Queued => {}
                Ahead::Refused => tracker.refuse(990_001),
                Ahead::Failed(mode) => tracker.set(mode),
            }
            if !matches!(fate, Ahead::Queued) {
                let report = media.drain(&store, &tracker, ahead).await;
                assert_eq!(report.sent, 0, "{case}");
            }

            tracker.set(Mode::Up);
            tracker.refused.lock().unwrap().clear();
            let (sent, dropped) = tick(media, &store, &tracker, 2_000, 2_000 + 3_600).await;
            assert_eq!(
                (sent, dropped),
                (1, 0),
                "{case}: the change goes once the clock is right"
            );
            assert_eq!(media.depth(&store), 0, "{case}");
        }
    }
}
