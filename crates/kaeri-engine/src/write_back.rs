//! Reading progress told back to the Komga and Kavita servers chapters came from, with Send
//! progress to the server on.
//!
//! Every mark goes into the store's queue first (`kaeri_store::book_progress`), so a mark made
//! offline, or one the server did not take, is kept across restarts. One row per chapter holds
//! the newest mark, and a row the server took is gone, so nothing is sent twice. The queue drains
//! once the launch has let go of background work, oldest mark first, a short rest between sends.
//! A failure is tried again after 30 s, 2 min, 10 min and then hourly; after a day of that it
//! waits for Try again as "no answer for a day". A refused key or password, and a series the
//! server no longer has, wait for Try again at once. Nothing leaves the queue on its own.
//!
//! Each server's row in Your sources gets the count waiting, the last success and the last
//! failure, and the reader gets a result for each chapter it reports.

use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use kaeri_core::{
    ProviderError,
    books::{BookHints, ChapterId, WorkId},
};
use kaeri_model::{
    book_sources::{WriteBackProblem, WriteBackResult},
    event::Event,
    reader::ReaderEvent,
};
use kaeri_store::book_progress::BookProgressItem;

use crate::{clock::Clock, outbox::Outbox, read_sources::BookSources};

/// Marks one pass sends at most.
const PASS: u32 = 50;

/// The rest between two sends to a server.
const PACE: Duration = Duration::from_millis(150);

/// The waits after the first, second and third failure, in seconds; hourly after that.
const RETRIES: [i64; 3] = [30, 120, 600];
const HOURLY: i64 = 3_600;

/// How long Kaeri keeps trying a server that does not answer before it waits for Try again.
const A_DAY: i64 = 86_400;

/// The longest the queue sleeps without being woken: a wait past it is checked again then.
const NAP: i64 = 3_600;

/// The wait after `attempts` failures.
pub fn retry_in(attempts: u32) -> i64 {
    attempts.checked_sub(1).and_then(|i| RETRIES.get(i as usize).copied()).unwrap_or(HOURLY)
}

/// What one pass did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pass {
    pub sent: u32,
    pub later: u32,
    pub held: u32,
}

type Key = (String, String, String);

/// A reader waiting to hear what became of a chapter it reported.
struct Waiter {
    tx: Outbox,
    work: WorkId,
    chapter: ChapterId,
    host: String,
}

/// The queue's state in memory. Cheap to clone; every clone shares it.
#[derive(Clone, Default)]
pub struct WriteBack(Arc<Inner>);

#[derive(Default)]
struct Inner {
    wake: tokio::sync::Notify,
    started: AtomicBool,
    offline: AtomicBool,
    sending: Mutex<HashSet<String>>,
    waiting: Mutex<HashMap<Key, Vec<Waiter>>>,
    publish: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

impl std::fmt::Debug for WriteBack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WriteBack")
            .field("started", &self.0.started.load(Ordering::Relaxed))
            .field("offline", &self.0.offline.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl WriteBack {
    fn wake(&self) {
        self.0.wake.notify_one();
    }

    fn is_offline(&self) -> bool {
        self.0.offline.load(Ordering::Relaxed)
    }

    /// Whether a send to `server` is under way.
    pub fn is_sending(&self, server: &str) -> bool {
        self.0.sending.lock().unwrap_or_else(|e| e.into_inner()).contains(server)
    }

    fn set_sending(&self, server: &str, on: bool) {
        let mut sending = self.0.sending.lock().unwrap_or_else(|e| e.into_inner());
        if on {
            sending.insert(server.to_owned());
        } else {
            sending.remove(server);
        }
    }

    /// How Your sources is told again after a pass changed a server's row.
    pub fn set_publisher(&self, publish: Arc<dyn Fn() + Send + Sync>) {
        *self.0.publish.lock().unwrap_or_else(|e| e.into_inner()) = Some(publish);
    }

    fn publish(&self) {
        let publish = self.0.publish.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if let Some(publish) = publish {
            publish();
        }
    }

    fn wait_for(&self, key: Key, waiter: Waiter) {
        self.0
            .waiting
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(key)
            .or_default()
            .push(waiter);
    }

    /// Tell the readers waiting on `key` what became of it.
    fn answer(&self, key: &Key, result: WriteBackResult) {
        let waiters = self.0.waiting.lock().unwrap_or_else(|e| e.into_inner()).remove(key);
        for w in waiters.into_iter().flatten() {
            let _ = w.tx.send(progress_sent(&w.work, &w.chapter, &key.0, &w.host, result));
        }
    }
}

fn progress_sent(
    work: &WorkId,
    chapter: &ChapterId,
    server: &str,
    host: &str,
    result: WriteBackResult,
) -> Event {
    Event::Reader(ReaderEvent::ProgressSent {
        work: work.clone(),
        chapter: chapter.clone(),
        server: server.to_owned(),
        host: host.to_owned(),
        result,
    })
}

/// The host a server's line names: "Saved to books.home.example".
fn host_of(address: &str) -> String {
    reqwest::Url::parse(address)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .unwrap_or_else(|| address.to_owned())
}

fn key_of(item: &BookProgressItem) -> Key {
    (item.server.clone(), item.series.clone(), item.chapter.clone())
}

/// Where a mark was queued, or why it was not.
enum Queued {
    /// The chapter is from no book server.
    Nowhere,
    /// The server is set not to receive progress.
    NotWanted {
        server: String,
        host: String,
    },
    Kept {
        server: String,
        series: String,
        host: String,
    },
}

impl BookSources {
    /// Queue `read` for `chapters` of `work` for the book server they come from, when the person
    /// lets Kaeri write to it, and wake the queue. A mark is kept until the server takes it.
    pub fn report(&self, work: &WorkId, chapters: Vec<ChapterId>, read: bool, now: i64) {
        if matches!(self.queue_report(work, &chapters, read, now), Queued::Kept { .. }) {
            self.write_back.wake();
        }
    }

    /// [`Self::report`] for one chapter read in the reader, which hears what became of it as
    /// [`ReaderEvent::ProgressSent`]: at once when it is known (offline, not set to receive, held
    /// for Try again), or when the server answers.
    pub fn report_read(&self, work: &WorkId, chapter: &ChapterId, now: i64, tx: &Outbox) {
        match self.queue_report(work, std::slice::from_ref(chapter), true, now) {
            Queued::Nowhere => {}
            Queued::NotWanted { server, host } => {
                let result = WriteBackResult::of(WriteBackProblem::NotSetToReceive);
                let _ = tx.send(progress_sent(work, chapter, &server, &host, result));
            }
            Queued::Kept { server, series, host } => {
                let held = self
                    .store()
                    .book_progress_item(&server, &series, chapter.as_str())
                    .ok()
                    .flatten()
                    .and_then(|i| i.held)
                    .and_then(|h| WriteBackProblem::parse(&h));
                let now_known =
                    held.or(self.write_back.is_offline().then_some(WriteBackProblem::Offline));
                if let Some(problem) = now_known {
                    let result = WriteBackResult::of(problem);
                    let _ = tx.send(progress_sent(work, chapter, &server, &host, result));
                    return;
                }
                let waiter = Waiter {
                    tx: tx.clone(),
                    work: work.clone(),
                    chapter: chapter.clone(),
                    host,
                };
                self.write_back.wait_for((server, series, chapter.as_str().to_owned()), waiter);
                self.write_back.wake();
            }
        }
    }

    /// The host a reader's line between chapters names for `work`: the book server its chapters
    /// come from, when that server receives progress.
    pub fn progress_host(&self, work: &WorkId) -> Option<String> {
        let (source, _) = self.edition_of(work, None)?;
        let server = self.store().book_server(&source).ok().flatten()?;
        server.report_progress.then(|| host_of(&server.address))
    }

    fn queue_report(
        &self,
        work: &WorkId,
        chapters: &[ChapterId],
        read: bool,
        now: i64,
    ) -> Queued {
        let Some((source, series)) = self.edition_of(work, None) else {
            return Queued::Nowhere;
        };
        let Some(server) = self.store().book_server(&source).ok().flatten() else {
            return Queued::Nowhere;
        };
        let host = host_of(&server.address);
        if !server.report_progress {
            return Queued::NotWanted { server: source, host };
        }
        for chapter in chapters {
            if let Err(e) = self.store().queue_book_progress(
                &source,
                &series,
                chapter.as_str(),
                read,
                now,
                now,
            ) {
                tracing::warn!(error = %e, "progress for a book server was not kept");
            }
        }
        Queued::Kept { server: source, series, host }
    }

    /// Start draining the queue, once: the launch has let go of background work. It runs while
    /// the engine does, woken by each report, Try again and the return of the network.
    pub fn start_write_back(&self, clock: Clock) {
        if self.write_back.0.started.swap(true, Ordering::AcqRel) {
            return;
        }
        let this = self.clone();
        tokio::spawn(async move {
            loop {
                let pass = this.send_progress_due(clock.now()).await;
                if pass != Pass::default() {
                    tracing::debug!(?pass, "book progress pass");
                }
                let now = clock.now();
                let next = this.store().next_book_progress_due().ok().flatten();
                let wait = match next {
                    _ if this.write_back.is_offline() => NAP,
                    Some(at) => (at - now).clamp(0, NAP),
                    None => NAP,
                };
                if wait == 0 {
                    // More was due than one pass sends.
                    tokio::task::yield_now().await;
                    continue;
                }
                let wait = Duration::from_secs(u64::try_from(wait).unwrap_or(1));
                tokio::select! {
                    () = tokio::time::sleep(wait) => {}
                    () = this.write_back.0.wake.notified() => {}
                }
            }
        });
    }

    /// The device went offline or came back. Back online, every mark waiting goes at once,
    /// oldest first.
    pub fn write_back_network(&self, offline: bool, now: i64) {
        let was = self.write_back.0.offline.swap(offline, Ordering::Relaxed);
        if was && !offline {
            if let Err(e) = self.store().book_progress_due_now(now) {
                tracing::warn!(error = %e, "book progress not brought forward");
            }
            self.write_back.wake();
        }
    }

    /// Try again: every mark for `server` goes now, the held ones too, and a refused sign-in is
    /// tried once more, as the key is often fixed on the server.
    pub fn retry_progress(&self, server: &str, now: i64) {
        self.forget_refusal(server);
        if let Err(e) = self.store().retry_book_progress(server, now) {
            tracing::warn!(error = %e, "book progress not retried");
        }
        self.write_back.wake();
    }

    /// The server is set not to receive progress, or removed: nothing waits for it.
    pub fn drop_progress(&self, server: &str) {
        if let Err(e) = self.store().forget_book_progress(server) {
            tracing::warn!(error = %e, "book progress not forgotten");
        }
    }

    /// The facts the server's row shows: waiting, sending, last saved, and why it waits.
    pub(crate) fn write_back_row(
        &self,
        server: &str,
        receives: bool,
    ) -> (u32, bool, Option<i64>, Option<WriteBackProblem>, Option<i64>) {
        let state = self.store().book_progress_state(server).unwrap_or_default();
        let kept = state.problem.as_deref().and_then(WriteBackProblem::parse);
        let problem = if !receives {
            Some(WriteBackProblem::NotSetToReceive)
        } else if state.pending == 0 {
            None
        } else if self.write_back.is_offline() && kept.is_none_or(WriteBackProblem::retries) {
            Some(WriteBackProblem::Offline)
        } else {
            kept
        };
        let problem_at =
            if problem.is_some() && problem == kept { state.problem_at } else { None };
        (state.pending, self.write_back.is_sending(server), state.sent_at, problem, problem_at)
    }

    /// Send the marks due at `now`, oldest first. Offline nothing is tried. A server that does
    /// not answer, or refuses the sign-in, is not asked again in the pass: its other marks wait
    /// with it.
    pub async fn send_progress_due(&self, now: i64) -> Pass {
        let mut pass = Pass::default();
        if self.write_back.is_offline() {
            return pass;
        }
        let due = match self.store().due_book_progress(now, PASS) {
            Ok(due) if !due.is_empty() => due,
            Ok(_) => return pass,
            Err(e) => {
                tracing::warn!(error = %e, "book progress queue unread");
                return pass;
            }
        };
        let mut groups: Vec<((String, String), Vec<BookProgressItem>)> = Vec::new();
        for item in due {
            let at = (item.server.clone(), item.series.clone());
            match groups.iter_mut().find(|(k, _)| *k == at) {
                Some((_, items)) => items.push(item),
                None => groups.push((at, vec![item])),
            }
        }
        let mut down: HashMap<String, WriteBackProblem> = HashMap::new();
        for ((server, series), items) in groups {
            if let Some(problem) = down.get(&server).copied() {
                for item in &items {
                    self.failed(item, problem, now, &mut pass);
                }
                continue;
            }
            let Some(row) = self.store().book_server(&server).ok().flatten() else {
                self.drop_progress(&server);
                continue;
            };
            if !row.report_progress {
                for item in &items {
                    self.write_back.answer(
                        &key_of(item),
                        WriteBackResult::of(WriteBackProblem::NotSetToReceive),
                    );
                }
                self.drop_progress(&server);
                continue;
            }
            self.write_back.set_sending(&server, true);
            self.write_back.publish();
            if let Some(problem) =
                self.send_group(&server, &series, &items, now, &mut pass).await
            {
                down.insert(server.clone(), problem);
            }
            self.write_back.set_sending(&server, false);
        }
        self.write_back.publish();
        pass
    }

    /// Send one series' marks to its server. A failure that will meet every other mark for the
    /// server is given back, and the rest of the series waits with it.
    async fn send_group(
        &self,
        server: &str,
        series: &str,
        items: &[BookProgressItem],
        now: i64,
        pass: &mut Pass,
    ) -> Option<WriteBackProblem> {
        let refused = self.refusal_of(server);
        let listed = if self.registry.get(server).is_some() {
            self.registry.chapters(server, series, &BookHints::default(), now).await
        } else {
            // Not in the registry: the sign-in is not kept.
            Err(ProviderError::Unavailable("signed out".into()))
        };
        let listed = match listed {
            Ok(listed) => listed,
            Err(e) => {
                let problem = problem_of(&e, refused);
                for item in items {
                    self.failed(item, problem, now, pass);
                }
                return whole_server(problem).then_some(problem);
            }
        };
        // A chapter the server lists in parts is marked part by part, all of them.
        let arranged = crate::read_catalog::arrange::arrange(&listed, None, None);
        for (i, item) in items.iter().enumerate() {
            let Some(row) =
                arranged.row(&kaeri_core::books::ChapterId::new(item.chapter.clone()))
            else {
                self.failed(item, WriteBackProblem::SeriesGone, now, pass);
                continue;
            };
            if i > 0 {
                tokio::time::sleep(PACE).await;
            }
            let mut answer = Ok(());
            for &part in &row.parts {
                answer = self
                    .registry
                    .report_progress(server, series, &listed[part].id, item.read, now)
                    .await;
                if answer.is_err() {
                    break;
                }
            }
            match answer {
                Ok(()) => {
                    pass.sent += 1;
                    if let Err(e) = self.store().book_progress_sent(item, now) {
                        tracing::warn!(error = %e, "a sent mark was not cleared");
                    }
                    self.write_back.answer(&key_of(item), WriteBackResult::Saved);
                }
                Err(e) => {
                    let problem = problem_of(&e, refused);
                    self.failed(item, problem, now, pass);
                    if whole_server(problem) {
                        for rest in &items[i + 1..] {
                            self.failed(rest, problem, now, pass);
                        }
                        return Some(problem);
                    }
                }
            }
        }
        None
    }

    /// Keep `item` for later, or for Try again, after `problem` at `now`.
    fn failed(
        &self,
        item: &BookProgressItem,
        problem: WriteBackProblem,
        now: i64,
        pass: &mut Pass,
    ) {
        let attempts = item.attempts + 1;
        let problem = if problem.retries()
            && attempts as usize > RETRIES.len()
            && now - item.first_failed_at.unwrap_or(now) >= A_DAY
        {
            WriteBackProblem::NoAnswerForADay
        } else {
            problem
        };
        let hold = !problem.retries();
        if hold {
            pass.held += 1;
        } else {
            pass.later += 1;
        }
        tracing::info!(problem = problem.as_str(), "a book server was not told what was read");
        if let Err(e) = self.store().book_progress_failed(
            item,
            problem.as_str(),
            hold,
            now + retry_in(attempts),
            now,
        ) {
            tracing::warn!(error = %e, "a failed mark was not kept");
        }
        self.write_back.answer(&key_of(item), WriteBackResult::of(problem));
    }
}

/// Whether `problem` meets every mark for the server, not just one chapter.
fn whole_server(problem: WriteBackProblem) -> bool {
    matches!(
        problem,
        WriteBackProblem::NoAnswer
            | WriteBackProblem::RefusedKey
            | WriteBackProblem::RefusedPassword
    )
}

/// What a source's error means for progress. `refused` is the server's kind of refusal: a key
/// or a password.
fn problem_of(error: &ProviderError, refused: WriteBackProblem) -> WriteBackProblem {
    match error {
        ProviderError::Transport(_) => WriteBackProblem::NoAnswer,
        ProviderError::Unavailable(why) if why == "signed out" => refused,
        ProviderError::NotFound => WriteBackProblem::SeriesGone,
        _ => WriteBackProblem::ServerError,
    }
}

#[cfg(test)]
mod tests;
