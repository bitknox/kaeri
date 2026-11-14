//! Send to e-reader and the OPDS feed: the engine's side of getting what is on this device
//! onto another reader.
//!
//! **Send to e-reader** prepares chapters for one target profile and hands back a
//! list of files for the app's share sheet: [`prepare`] writes them, [`profiles`] says what a
//! target is. A chapter that is not on this device is fetched through the download pipeline for
//! the send and removed once its files are written; downloaded ones and files in the person's
//! folders are used as they are. The job is announced like any other, told as `ExportProgress`
//! and ended with `ExportReady`; it stops on `CancelPrepare` and leaves no files behind. A send
//! with chapters to fetch waits while the downloads do, and goes on by itself: for Wi-Fi it says
//! so with `ExportWaiting`, and one left waiting when the engine stops starts again at the next
//! launch ([`held`]). A ready send is kept beside its files, so one ready when the app stopped is
//! offered again at the next launch with `ExportKept` ([`kept`]). Until the front end first says the network it waits too, without a word,
//! as the network may be cellular. With no network at all it fails as offline. Once the files are shared, the series page asks
//! where you stopped, and the answer is a `ReadAdopt` op.
//!
//! **The OPDS feed** is in [`feed`], behind the `opds-feed` feature the desktop builds turn on.

pub(crate) mod held;
pub(crate) mod kept;
pub mod prepare;
pub mod profiles;

#[cfg(feature = "opds-feed")]
pub mod feed;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};

use kaeri_core::books::{ChapterId, WorkId};
use kaeri_model::{
    Msg, ReadingMsg,
    event::Event,
    jobs::JobId,
    reading::{
        DownloadPick, ExportFormat, FeedView, ReadingAbout, ReadingCommand, ReadingEvent,
        ReadingFailure, VolumeGroup,
    },
};
use kaeri_store::{Store, book_downloads::BookDownloadState};

use crate::{
    downloads::books::{
        BookDownloads, Fetching, Here, Lent, OnDevice, Stop as FetchStop, Wanted, chapter_row,
        flatten,
    },
    jobs::JobGuard,
    outbox::Outbox,
    services::Services,
    state::EngineState,
};

/// How long the files of a ready send are kept for its Share, in seconds from when it became
/// ready. Sharing does not end it, as a ready send can be shared again until it is put away.
const READY_KEPT: i64 = 24 * 60 * 60;

/// The sends under way, so one can be stopped, the ready ones whose files wait for the share
/// sheet, and where their files go.
///
/// Job ids start over at every launch, so a send's folders are named by this engine's own mark
/// as well as its job: `<mark>-<job>` for its files and `<mark>-<job>-fetch` for the pages it
/// fetches. A folder an earlier launch left is never taken for one of this launch's: [`sweep`]
/// removes it at launch (a send killed mid-fetch, a ready send no longer kept), except a ready
/// send still kept, which the launch offers again ([`kept`]).
#[derive(Clone, Debug)]
pub struct Exports {
    dir: PathBuf,
    /// This engine's mark: see [`mark`].
    mark: Arc<str>,
    running: Arc<Mutex<HashMap<u64, Arc<AtomicBool>>>>,
    /// Ready sends by job, with when each became ready on the engine's clock and the name of
    /// its folder: its own, or for a send kept from an earlier launch, the one that launch made.
    /// Sends that joined one share its folder.
    ready: Arc<Mutex<HashMap<u64, (i64, String)>>>,
    /// The sends running by what they send (work, chapters, target and format), each with the
    /// jobs of the identical sends asked for while it ran, which join it rather than fetch the
    /// same chapters again.
    joins: Arc<Mutex<Joins>>,
}

/// The sends running by what they send, each with its own job and the jobs that joined it.
type Joins = HashMap<String, (u64, Arc<Joined>)>;

/// The jobs that joined a running send: each is told what it is told, and ends when it ends.
/// When its own job is put away while others joined it, it runs on for them alone.
#[derive(Default)]
struct Joined {
    jobs: Mutex<Vec<(JobId, Option<JobGuard>)>>,
    /// Its own job was put away; set only with `jobs` locked.
    lead_gone: AtomicBool,
    /// What the send said while it waits for Wi-Fi, for a send that joins it then to be told
    /// as well; set and read only with `jobs` locked.
    waiting: Mutex<Option<Waits>>,
}

/// The line a send waiting for Wi-Fi shows, and the chapters it holds.
type Waits = (Msg, Vec<ChapterId>);

impl std::fmt::Debug for Joined {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let jobs: Vec<u64> = self.lock().iter().map(|(job, _)| job.get()).collect();
        f.debug_tuple("Joined").field(&jobs).finish()
    }
}

impl Joined {
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<(JobId, Option<JobGuard>)>> {
        self.jobs.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn lead_gone(&self) -> bool {
        self.lead_gone.load(Ordering::Relaxed)
    }

    fn jobs(&self) -> Vec<JobId> {
        self.lock().iter().map(|(job, _)| *job).collect()
    }

    fn waiting(&self) -> std::sync::MutexGuard<'_, Option<Waits>> {
        self.waiting.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// What a send sends, so two identical ones are told apart from the rest.
fn identity(work: &WorkId, pick: &DownloadPick, target: &str, format: ExportFormat) -> String {
    format!("{}\u{1f}{pick:?}\u{1f}{target}\u{1f}{format:?}", work.as_str())
}

/// A mark no other engine has: the process id, then when it was made and a count, as
/// `<pid>.<hex>`. The process id comes first so the launch sweep can tell a folder of this
/// process (an engine still running) from one of a process that has ended.
fn mark() -> String {
    static MADE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let count = MADE.fetch_add(1, Ordering::Relaxed);
    format!("{}.{nanos:x}{count:x}", std::process::id())
}

/// Whether an entry of the sends' folder belongs to an engine of this process. Any other (a
/// send of a process that has ended, or a folder named before marks) is a leftover.
fn of_this_process(name: &str) -> bool {
    name.split_once('.').is_some_and(|(pid, _)| pid == std::process::id().to_string())
}

/// Where sends write their files and fetch their pages.
pub fn dir_of(paths: &kaeri_core::config::Paths) -> PathBuf {
    paths.data_dir.join("exports")
}

/// Remove what sends of earlier launches left in `dir`, the sends' folder: ready files no
/// launch offers any more (put away, past [`READY_KEPT`] or missing a file) and the pages of
/// sends killed mid-fetch. Folders of this process's engines stay, and so do kept ready sends,
/// which the launch offers again. Blocking. Returns how many entries went.
pub fn sweep(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    let now = kept::wall_now();
    let mut removed = 0;
    for entry in entries.flatten() {
        if of_this_process(&entry.file_name().to_string_lossy()) {
            continue;
        }
        let path = entry.path();
        if kept::KeptSend::read(&path, now).is_some() {
            continue;
        }
        let gone = match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(&path),
            Ok(_) => std::fs::remove_file(&path),
            Err(e) => Err(e),
        };
        if gone.is_ok() {
            removed += 1;
        }
    }
    removed
}

impl Exports {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            mark: mark().into(),
            running: Arc::default(),
            ready: Arc::default(),
            joins: Arc::default(),
        }
    }

    /// The name of `job`'s folder: this engine's mark and the job.
    fn name(&self, job: JobId) -> String {
        format!("{}-{}", self.mark, job.get())
    }

    /// Where `job` keeps the pages it fetches, removed once its files are written.
    fn scratch(&self, job: JobId) -> PathBuf {
        self.dir.join(format!("{}-fetch", self.name(job)))
    }

    fn begin(&self, job: JobId) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        self.running.lock().unwrap_or_else(|e| e.into_inner()).insert(job.get(), flag.clone());
        flag
    }

    /// A send's files are written: they are kept for its Share, see [`Self::folder`].
    fn ready(&self, job: JobId, now: i64) {
        let name = self.name(job);
        self.ready.lock().unwrap_or_else(|e| e.into_inner()).insert(job.get(), (now, name));
    }

    /// The ready sends earlier launches kept in the sends' folder and no job of this engine has
    /// taken yet, with their folders' names, oldest first.
    fn kept(&self) -> Vec<(String, kept::KeptSend)> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else { return Vec::new() };
        let taken: Vec<String> = self
            .ready
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .map(|(_, name)| name.clone())
            .collect();
        let now = kept::wall_now();
        let mut found: Vec<(String, kept::KeptSend)> = entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                let ours = name.starts_with(&format!("{}-", self.mark));
                if ours || taken.contains(&name) {
                    return None;
                }
                Some((name, kept::KeptSend::read(&entry.path(), now)?))
            })
            .collect();
        found.sort_by_key(|(_, send)| send.at);
        found
    }

    /// `job` of this launch is the ready send kept in the folder `name`, ready since `at` on
    /// the engine's clock.
    fn adopt(&self, job: JobId, name: String, at: i64) {
        self.ready.lock().unwrap_or_else(|e| e.into_inner()).insert(job.get(), (at, name));
    }

    fn end(&self, job: JobId) {
        self.running.lock().unwrap_or_else(|e| e.into_inner()).remove(&job.get());
    }

    /// Stops a send; false when it is not running. A ready send is put away instead: its
    /// files are removed, as the screen that asked has dropped it.
    pub fn cancel(&self, job: JobId) -> bool {
        let flag =
            self.running.lock().unwrap_or_else(|e| e.into_inner()).get(&job.get()).cloned();
        if let Some(flag) = flag {
            // A send others joined runs on for them; only its own job leaves.
            let followed = self
                .joins
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .values()
                .find(|(lead, _)| *lead == job.get())
                .map(|(_, joined)| joined.clone());
            if let Some(joined) = followed {
                let jobs = joined.lock();
                if !jobs.is_empty() {
                    joined.lead_gone.store(true, Ordering::Relaxed);
                    return true;
                }
            }
            flag.store(true, Ordering::Relaxed);
            return true;
        }
        // A send that joined another leaves it; the other goes on, unless no one is left.
        let mut left = None;
        for (lead, joined) in self.joins.lock().unwrap_or_else(|e| e.into_inner()).values() {
            let mut jobs = joined.lock();
            if let Some(at) = jobs.iter().position(|(j, _)| *j == job) {
                jobs.remove(at);
                left = Some((*lead, jobs.is_empty() && joined.lead_gone()));
                break;
            }
        }
        if let Some((lead, last)) = left {
            let running = self.running.lock().unwrap_or_else(|e| e.into_inner());
            if last && let Some(flag) = running.get(&lead) {
                flag.store(true, Ordering::Relaxed);
            }
            return true;
        }
        let mut ready = self.ready.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, name)) = ready.remove(&job.get()) {
            // The files go once no send that joined it still offers them.
            if !ready.values().any(|(_, other)| *other == name) {
                let _ = std::fs::remove_dir_all(self.dir.join(name));
            }
        }
        false
    }

    /// Join the running send identical to the one `job` asks for, when there is one: `job` is
    /// then told what that send is told and ends with it, and `Ok` says so, with what the send
    /// said when it waits for Wi-Fi already, which `job` is to be told too. Otherwise the guard
    /// comes back for a send of its own.
    fn join(
        &self,
        identity: &str,
        job: Option<JobGuard>,
    ) -> Result<Option<Waits>, Option<JobGuard>> {
        let joins = self.joins.lock().unwrap_or_else(|e| e.into_inner());
        let (Some((_, joined)), Some(id)) = (joins.get(identity), job.as_ref().map(|j| j.id))
        else {
            return Err(job);
        };
        let mut jobs = joined.lock();
        jobs.push((id, job));
        let waits = joined.waiting().clone();
        Ok(waits)
    }

    /// `job` runs the send `identity` names, which identical ones join while it runs.
    fn lead(&self, identity: &str, job: JobId) -> Arc<Joined> {
        let joined = Arc::new(Joined::default());
        let mut joins = self.joins.lock().unwrap_or_else(|e| e.into_inner());
        joins.insert(identity.to_owned(), (job.get(), joined.clone()));
        joined
    }

    /// The send `identity` names has ended: none joins it any more.
    fn unlead(&self, identity: &str, job: JobId) {
        let mut joins = self.joins.lock().unwrap_or_else(|e| e.into_inner());
        if joins.get(identity).is_some_and(|(lead, _)| *lead == job.get()) {
            joins.remove(identity);
        }
    }

    /// `job` joined a send that is ready now: it offers the same files.
    fn ready_as(&self, job: JobId, lead: JobId, now: i64) {
        let name = self.name(lead);
        self.ready.lock().unwrap_or_else(|e| e.into_inner()).insert(job.get(), (now, name));
    }

    /// The folder one send writes to. The folder's other entries are removed, except those of
    /// sends still running and of ready ones younger than [`READY_KEPT`], which a Share may
    /// still hand over: older ready sends, and leftovers such as an earlier launch's sends and
    /// scratch folders, go.
    fn folder(&self, job: JobId, now: i64) -> PathBuf {
        let mut keep: Vec<String> = self
            .running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .keys()
            .map(|k| self.name(JobId::new(*k)))
            .collect();
        {
            let mut ready = self.ready.lock().unwrap_or_else(|e| e.into_inner());
            ready.retain(|_, (at, _)| now.saturating_sub(*at) < READY_KEPT);
            keep.extend(ready.values().map(|(_, name)| name.clone()));
        }
        if let Ok(entries) = std::fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                // A send's scratch folder is its name and `-fetch`.
                let kept = keep
                    .iter()
                    .any(|b| name == *b || name.strip_suffix("-fetch") == Some(b.as_str()));
                if !kept {
                    let _ = std::fs::remove_dir_all(entry.path());
                }
            }
        }
        self.dir.join(self.name(job))
    }
}

/// The commands this module answers.
pub(crate) fn handles(command: &ReadingCommand) -> bool {
    matches!(
        command,
        ReadingCommand::LoadExportTargets
            | ReadingCommand::Prepare { .. }
            | ReadingCommand::CancelPrepare { .. }
            | ReadingCommand::Shared { .. }
            | ReadingCommand::StoppedAt { .. }
            | ReadingCommand::SetSendTarget { .. }
            | ReadingCommand::LoadFeed
            | ReadingCommand::SetFeed { .. }
            | ReadingCommand::SetFeedShare { .. }
    )
}

/// The targets the apps list, the one last sent to marked.
fn targets_of(engine: &EngineState) -> Vec<kaeri_model::reading::ExportTarget> {
    let reading = &engine.config.effective().reading;
    let chosen = reading.send_target.as_deref();
    profiles::all(reading)
        .iter()
        .map(|p| {
            let mut target = p.target();
            target.chosen = chosen == Some(target.id.as_str());
            target
        })
        .collect()
}

fn fail(about: ReadingAbout, failure: ReadingFailure, tx: &Outbox) {
    let _ = tx.send(Event::Reading(ReadingEvent::Failed { about, failure }));
}

/// A send or feed command.
pub(crate) fn command(
    engine: &mut EngineState,
    services: &Services,
    command: ReadingCommand,
    job: Option<JobGuard>,
    tx: &Outbox,
) {
    let about = command.about();
    if !engine.reading.available {
        return crate::reading::off(about, tx);
    }
    let sources = &services.book_sources;
    match command {
        ReadingCommand::LoadExportTargets => {
            let _ = tx.send(Event::Reading(ReadingEvent::ExportTargets(targets_of(engine))));
        }
        ReadingCommand::SetSendTarget { target } => {
            use kaeri_core::settings::SettingValue;
            // In effect before it is written, as a setting is: `write` keeps only the file.
            engine.config.effective_mut().reading.send_target = Some(target.clone());
            if let Err(e) = engine.config.write(
                &services.paths,
                &["reading"],
                "send_target",
                SettingValue::Str(target),
            ) {
                tracing::warn!(error = %e, "the send target was not kept");
                let kept = engine.config.saved().reading.send_target.clone();
                engine.config.effective_mut().reading.send_target = kept;
            }
            let _ = tx.send(Event::Reading(ReadingEvent::ExportTargets(targets_of(engine))));
        }
        ReadingCommand::Prepare { work, pick, target, format } => {
            prepare_files(engine, services, work, pick, &target, format, job, tx);
        }
        ReadingCommand::CancelPrepare { job } => {
            sources.exports.cancel(job);
        }
        ReadingCommand::Shared { work, chapters, target } => {
            let ids: Vec<String> = chapters.iter().map(|c| c.as_str().to_owned()).collect();
            let now = services.clock.now();
            if let Err(e) = sources.store().record_sent(work.as_str(), &ids, &target, now) {
                tracing::warn!(error = %e, "a send was not kept");
            }
            series_header(engine, services, &work, tx);
        }
        ReadingCommand::StoppedAt { work, chapter, page: _ } => {
            let store = sources.store();
            if let Some(chapter) = chapter {
                // Everything up to where you stopped counts as read: a ReadAdopt.
                let now = services.clock.now();
                if let Err(e) = store.adopt_read(work.as_str(), chapter.as_str(), None, now) {
                    tracing::warn!(error = %e, "where you stopped was not kept");
                }
            }
            if let Err(e) = store.answer_sent(work.as_str()) {
                tracing::warn!(error = %e, "an answered send was not marked");
            }
            series_header(engine, services, &work, tx);
        }
        ReadingCommand::LoadFeed => {
            let _ = tx.send(Event::Reading(ReadingEvent::Feed(feed_view(engine, services))));
        }
        ReadingCommand::SetFeed { on, password } => {
            set_feed(engine, services, on, password.map(|p| p.0), tx);
        }
        ReadingCommand::SetFeedShare { shelf, on } => {
            set_feed_share(engine, services, shelf, on, tx)
        }
        _ => {}
    }
}

fn series_header(engine: &EngineState, services: &Services, work: &WorkId, tx: &Outbox) {
    let kit = crate::read_catalog::Kit::of(services);
    let ask = crate::read_catalog::Ask::of(engine, services);
    crate::read_catalog::series::header(&kit, ask, work, tx);
}

// ---------------------------------------------------------------------------------------------
// Send to e-reader

/// The downloaded chapters `pick` names, in reading order, with the file each is.
pub fn chosen(store: &Store, work: &str, pick: &DownloadPick) -> Vec<prepare::Chapter> {
    let marks = store.read_marks(work).unwrap_or_default();
    let done: Vec<_> = store
        .book_downloads(Some(work))
        .unwrap_or_default()
        .into_iter()
        .filter(|d| d.state == BookDownloadState::Done && d.path.is_some())
        .collect();
    let chapters = done
        .iter()
        .map(|d| chapter_row(d, marks.is_read(&d.chapter), false))
        .collect::<Vec<_>>();
    let rows = flatten(vec![VolumeGroup {
        volume: None,
        title: None,
        chapters,
        all_read: false,
        flat: true,
    }]);
    BookDownloads::picked(pick, &rows)
        .into_iter()
        .filter_map(|want| {
            let row = rows.iter().find(|r| r.id == want.chapter)?;
            let file = done.iter().find(|d| d.chapter == want.chapter.as_str())?;
            Some(prepare::Chapter {
                label: row.number.clone().unwrap_or_else(|| row.label.clone()),
                path: PathBuf::from(file.path.as_ref()?),
            })
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn prepare_files(
    engine: &EngineState,
    services: &Services,
    work: WorkId,
    pick: DownloadPick,
    target: &str,
    format: ExportFormat,
    job: Option<JobGuard>,
    tx: &Outbox,
) {
    let about = ReadingAbout::Export;
    let reading = &engine.config.effective().reading;
    let Some(profile) = profiles::find(reading, target) else {
        return fail(about, ReadingFailure::NotFound, tx);
    };
    let store = services.book_sources.store();
    let form = crate::data::title_form(services.meta.titles());
    let title = crate::read_catalog::tiles::title_of(store, work.as_str(), form);
    let direction = page_direction(engine, services, &work);
    let id = job.as_ref().map_or(JobId::new(0), |j| j.id);
    let exports = services.book_sources.exports.clone();
    // An identical send already running is joined, so its chapters are fetched once.
    let identity = identity(&work, &pick, target, format);
    let job = match exports.join(&identity, job) {
        Ok(waits) => {
            // It waits for Wi-Fi already, and said so before this send joined it.
            if let Some((line, chapters)) = waits {
                let target = target.to_owned();
                let waiting =
                    ReadingEvent::ExportWaiting { job: id, work, target, line, chapters };
                let _ = tx.send(Event::Reading(waiting));
            }
            return;
        }
        Err(job) => job,
    };
    let joined = exports.lead(&identity, id);
    let cancel = exports.begin(id);
    let out = exports.folder(id, services.clock.now());
    let clock = services.clock.clone();
    let send = OneSend {
        key: id.get().to_string(),
        target: target.to_owned(),
        lent: Lent::of(services, tx),
        downloads: services.book_sources.downloads.clone(),
        exports,
        told: Told::new(tx.clone(), id, joined.clone()),
        cancel,
        id,
        work,
        pick,
        title,
        profile,
        format,
        out,
        direction,
        sent: Mutex::default(),
    };
    tokio::spawn(async move {
        let _job = job;
        let tx = send.lent.tx.clone();
        let result = send.run().await;
        // No send joins this one from here on; those that did end with it.
        send.exports.unlead(&identity, send.id);
        let (also, gone) = {
            let mut jobs = joined.lock();
            (std::mem::take(&mut *jobs), joined.lead_gone())
        };
        let jobs: Vec<JobId> = (!gone)
            .then_some(send.id)
            .into_iter()
            .chain(also.iter().map(|(job, _)| *job))
            .collect();
        if let Ok(files) = &result {
            let now = clock.now();
            if !gone {
                send.exports.ready(send.id, now);
            }
            for (job, _) in &also {
                send.exports.ready_as(*job, send.id, now);
            }
            send.keep_ready(files);
        }
        send.exports.end(send.id);
        let chapter = send.told.failing_chapter();
        for job in jobs {
            let said = match &result {
                Ok(files) => ReadingEvent::ExportReady { job, files: files.clone() },
                Err(prepare::Stop::Cancelled) => continue,
                Err(prepare::Stop::Failed(failure)) => ReadingEvent::ExportFailed {
                    job,
                    chapter: chapter.clone(),
                    failure: failure.clone(),
                },
            };
            let _ = tx.send(Event::Reading(said));
        }
        drop(also);
    });
}

/// Which way the series' pages turn on the e-reader: as the reader turns them, from the same
/// layers (the person's choice for the series, its source's word, the reader's kept guess, the
/// medium's default). A series read as a strip or scrolled turns its pages forward.
fn page_direction(
    engine: &EngineState,
    services: &Services,
    work: &WorkId,
) -> prepare::PageDirection {
    use kaeri_model::reader::ReaderSettingKey as Key;

    use crate::reading::settings::{Layers, Screen, SourceWord};
    let layers = Layers::read(
        services.book_sources.store(),
        &engine.config.effective().reading,
        kaeri_core::media::Medium::Manga,
        Some(work.as_str()),
        Screen::default(),
    )
    .with_source(SourceWord::of(services, work));
    let flows = matches!(layers.word(Key::Mode).as_str(), "strip" | "vertical");
    let right_to_left = !flows && layers.word(Key::Direction) == "rtl";
    let said = flows || layers.series_has(Key::Direction) || layers.sourced(Key::Direction);
    prepare::PageDirection { said: said.then_some(right_to_left), otherwise: right_to_left }
}

/// Progress of one send, told at most every 150 ms except for its end.
struct Told {
    tx: Outbox,
    job: JobId,
    last: Mutex<Instant>,
    /// Chapters gathered so far, and how many there are to gather.
    gathered: AtomicU32,
    of: AtomicU32,
    /// The chapter being fetched, for a failure to name.
    current: Mutex<Option<String>>,
    /// The identical sends that joined this one, told the same.
    joined: Arc<Joined>,
}

impl Told {
    fn new(tx: Outbox, job: JobId, joined: Arc<Joined>) -> Arc<Self> {
        Arc::new(Self {
            tx,
            job,
            joined,
            last: Mutex::new(Instant::now() - Duration::from_secs(1)),
            gathered: AtomicU32::new(0),
            of: AtomicU32::new(0),
            current: Mutex::new(None),
        })
    }

    /// This send's job, unless it was put away, and those of the sends that joined it.
    fn jobs(&self) -> Vec<JobId> {
        let own = (!self.joined.lead_gone()).then_some(self.job);
        own.into_iter().chain(self.joined.jobs()).collect()
    }

    /// The send waits for Wi-Fi, having said `waits`, or no longer does with `None`: a send
    /// that joins it while it waits is told the same. Gives the jobs to tell now.
    fn waits(&self, waits: Option<Waits>) -> Vec<JobId> {
        let jobs = self.joined.lock();
        *self.joined.waiting() = waits;
        let own = (!self.joined.lead_gone()).then_some(self.job);
        own.into_iter().chain(jobs.iter().map(|(job, _)| *job)).collect()
    }

    fn chapter(&self, label: Option<String>) {
        *self.current.lock().unwrap_or_else(|e| e.into_inner()) = label;
    }

    fn failing_chapter(&self) -> Option<String> {
        self.current.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn tell(&self, done: u32, total: u32) {
        let mut last = self.last.lock().unwrap_or_else(|e| e.into_inner());
        if done < total && last.elapsed() < Duration::from_millis(150) {
            return;
        }
        *last = Instant::now();
        for job in self.jobs() {
            let _ = self.tx.send(Event::Reading(ReadingEvent::ExportProgress {
                job,
                done,
                total,
                chapters_done: self.gathered.load(Ordering::Relaxed),
                chapters: self.of.load(Ordering::Relaxed),
            }));
        }
    }
}

/// How much of the progress bar the fetching takes, per chapter fetched. Preparing the pages
/// takes as much again.
const FETCH_WEIGHT: u32 = 100;

/// One send, from picking the chapters to the files.
struct OneSend {
    /// Which send this is in the list of those waiting: its job, unique in a launch.
    key: String,
    target: String,
    lent: Lent,
    downloads: BookDownloads,
    exports: Exports,
    told: Arc<Told>,
    cancel: Arc<AtomicBool>,
    id: JobId,
    work: WorkId,
    pick: DownloadPick,
    title: String,
    profile: profiles::Profile,
    format: ExportFormat,
    out: PathBuf,
    direction: prepare::PageDirection,
    /// The chapters gathered, in reading order, for the record a ready send keeps.
    sent: Mutex<Vec<kaeri_core::books::ChapterId>>,
}

impl OneSend {
    async fn run(&self) -> Result<Vec<kaeri_model::reading::ExportFile>, prepare::Stop> {
        let temp = self.exports.scratch(self.id);
        let gathered = self.gather(&temp).await;
        let result = match gathered {
            Ok((chapters, fetched)) => self.write(chapters, fetched).await,
            Err(stop) => Err(stop),
        };
        // What was fetched only for this send goes, whether it worked or not.
        let _ = std::fs::remove_dir_all(&temp);
        if result.is_err() {
            let _ = std::fs::remove_dir_all(&self.out);
        }
        result
    }

    async fn write(
        &self,
        chapters: Vec<prepare::Chapter>,
        fetched: u32,
    ) -> Result<Vec<kaeri_model::reading::ExportFile>, prepare::Stop> {
        let first = chapters.first().map(|c| c.label.clone()).unwrap_or_default();
        let last = chapters.last().map(|c| c.label.clone()).unwrap_or_default();
        let request = prepare::Request {
            identifier: format!("urn:kaeri:{}:{first}-{last}", self.work.as_str()),
            title: self.title.clone(),
            chapters,
            profile: self.profile.clone(),
            format: self.format,
            out: self.out.clone(),
            limit: None,
            direction: self.direction,
        };
        let (told, cancel) = (self.told.clone(), self.cancel.clone());
        let blocking = tokio::task::spawn_blocking(move || {
            let mut progress = |done: u32, total: u32| {
                if fetched == 0 || total == 0 {
                    return told.tell(done, total);
                }
                // The second half of the bar: the fetching has filled the first.
                let half = fetched * FETCH_WEIGHT;
                let at = u64::from(done) * u64::from(half) / u64::from(total);
                told.tell(half + u32::try_from(at).unwrap_or(half), 2 * half);
            };
            prepare::run(&request, &mut progress, &cancel)
        });
        blocking.await.unwrap_or(Err(prepare::Stop::Failed(ReadingFailure::Unreadable {
            path: String::new(),
            format: "send".into(),
        })))
    }

    /// The chapters to send, in reading order, each as a file: the downloaded ones as they
    /// are, the others fetched through the download pipeline into `temp`. Also says how many
    /// were fetched.
    async fn gather(&self, temp: &Path) -> Result<(Vec<prepare::Chapter>, u32), prepare::Stop> {
        let store = self.lent.sources.store();
        let here = OnDevice::of(store, self.work.as_str());
        let asked_all_downloaded = match &self.pick {
            DownloadPick::Chapters { ids } => {
                !ids.is_empty()
                    && ids
                        .iter()
                        .all(|c| matches!(here.file(c.as_str()), Some(Here::Downloaded { .. })))
            }
            _ => false,
        };
        if asked_all_downloaded {
            // Nothing to fetch, so nothing to ask a source.
            if let DownloadPick::Chapters { ids } = &self.pick {
                *self.sent.lock().unwrap_or_else(|e| e.into_inner()) = ids.clone();
            }
            let chapters = chosen(store, self.work.as_str(), &self.pick);
            let n = u32::try_from(chapters.len()).unwrap_or(u32::MAX);
            self.told.of.store(n, Ordering::Relaxed);
            self.told.gathered.store(n, Ordering::Relaxed);
            return Ok((chapters, 0));
        }
        // A send held over from the last run starts at launch, maybe before the plugins are
        // in: they are waited for, so the source is asked rather than missed.
        self.lent.sources.plugins_in(&self.lent.plugins, None).await;
        self.lent.sources.adopt_plugins(&self.lent.plugins);
        let now = self.lent.clock.now();
        let (source, rows) =
            crate::downloads::books::ordered_chapters(&self.lent.sources, &self.work, now)
                .await
                .map_err(prepare::Stop::Failed)?;
        // Each chapter comes from the source that lists it; one no source lists any more is
        // left out, as a download of it would fail.
        let listers =
            crate::downloads::books::Listers::of(&self.lent.sources, &self.work, &source);
        let rows: Vec<_> = rows.into_iter().filter(|r| listers.of_row(r).is_some()).collect();
        let wanted = BookDownloads::picked(&self.pick, &rows);
        if wanted.is_empty() {
            return Err(ReadingFailure::NotFound.into());
        }
        *self.sent.lock().unwrap_or_else(|e| e.into_inner()) =
            wanted.iter().map(|w| w.chapter.clone()).collect();
        // A chapter on this device, downloaded or in the person's folders, is sent from its
        // file; only the rest are fetched, and only they wait for the network.
        let file_of = |want: &Wanted| here.file(want.chapter.as_str());
        let to_fetch = wanted.iter().filter(|w| file_of(w).is_none()).count();
        let fetch_count = u32::try_from(to_fetch).unwrap_or(u32::MAX);
        let total = fetch_count.saturating_mul(2 * FETCH_WEIGHT);
        let mut chapters = Vec::with_capacity(wanted.len());
        let mut fetched = 0u32;
        self.told.of.store(u32::try_from(wanted.len()).unwrap_or(u32::MAX), Ordering::Relaxed);
        for want in &wanted {
            if self.cancel.load(Ordering::Relaxed) {
                return Err(prepare::Stop::Cancelled);
            }
            let Some(row) = rows.iter().find(|r| r.id == want.chapter) else { continue };
            let label = row.number.clone().unwrap_or_else(|| row.label.clone());
            if let Some(file) = file_of(want) {
                chapters.push(prepare::Chapter { label, path: file.path().to_owned() });
                self.told.gathered.fetch_add(1, Ordering::Relaxed);
                continue;
            }
            self.told.chapter(Some(label.clone()));
            // A download of the chapter already under way is waited for and its file taken,
            // so the source is asked for its pages once.
            if let Some(path) = self.joined(want).await? {
                chapters.push(prepare::Chapter { label, path });
                self.told.gathered.fetch_add(1, Ordering::Relaxed);
                self.told.chapter(None);
                fetched += 1;
                continue;
            }
            let missing = u32::try_from(to_fetch).unwrap_or(u32::MAX).saturating_sub(fetched);
            self.may_fetch(missing, &wanted).await?;
            std::fs::create_dir_all(temp).map_err(|e| {
                prepare::Stop::Failed(crate::downloads::books::io_failure(&e, temp))
            })?;
            let base = fetched * FETCH_WEIGHT;
            let told = self.told.clone();
            let cancel = self.cancel.clone();
            let wanted_now = move || !cancel.load(Ordering::Relaxed);
            let progress = move |done: u32, pages: u32| {
                let part = u64::from(done) * u64::from(FETCH_WEIGHT) / u64::from(pages.max(1));
                told.tell(base + u32::try_from(part).unwrap_or(0), total);
            };
            let job = Fetching {
                work: self.work.clone(),
                chapter: want.chapter.clone(),
                source_id: listers.of_row(row).unwrap_or(&source).to_owned(),
                stem: temp.join(crate::downloads::books::safe(want.chapter.as_str())),
                wanted: &wanted_now,
                progress: &progress,
            };
            let file = loop {
                match self.downloads.fetch_with(&self.lent, &job).await {
                    Ok(file) => break file,
                    Err(FetchStop::Cancelled) => return Err(prepare::Stop::Cancelled),
                    // The network changed under the fetch: wait, then fetch the chapter again.
                    Err(FetchStop::Held) => self.may_fetch(missing, &wanted).await?,
                    Err(FetchStop::Failed(failure)) => {
                        return Err(prepare::Stop::Failed(failure));
                    }
                }
            };
            chapters.push(prepare::Chapter { label, path: file.path });
            self.told.gathered.fetch_add(1, Ordering::Relaxed);
            self.told.chapter(None);
            fetched += 1;
        }
        Ok((chapters, fetch_count))
    }

    /// The file of a download of `want` that is fetching, once it is done, or one that has
    /// finished since the send began. `None` when no download of it is fetching, or the one
    /// that was stops without the file (it failed, was cancelled or waits again): the send
    /// then fetches the chapter itself.
    async fn joined(&self, want: &Wanted) -> Result<Option<PathBuf>, prepare::Stop> {
        let store = self.lent.sources.store();
        let (work, chapter) = (self.work.as_str(), want.chapter.as_str());
        loop {
            if self.cancel.load(Ordering::Relaxed) {
                return Err(prepare::Stop::Cancelled);
            }
            let state = store.book_download(work, chapter).ok().flatten().map(|d| d.state);
            match state {
                Some(BookDownloadState::Downloading) => {}
                Some(BookDownloadState::Done) => {
                    let here = OnDevice::of(store, work);
                    return Ok(here.file(chapter).map(|file| file.path().to_owned()));
                }
                _ => return Ok(None),
            }
            tokio::time::sleep(LOOK_AGAIN).await;
        }
    }

    /// Wait until chapters may be fetched, `missing` of them still to come. A send waits while
    /// the downloads do: for Wi-Fi it says so, naming the chapters it holds (`wanted`), and it
    /// is kept in the store meanwhile. With no network at all it fails as offline; cancelled,
    /// it stops.
    async fn may_fetch(&self, missing: u32, wanted: &[Wanted]) -> Result<(), prepare::Stop> {
        let store = self.lent.sources.store();
        let mut changes = self.downloads.changes();
        let mut kept = false;
        let mut said = false;
        let result = loop {
            if self.cancel.load(Ordering::Relaxed) {
                break Err(prepare::Stop::Cancelled);
            }
            let setup = self.downloads.setup();
            if !setup.hold {
                break Ok(());
            }
            if setup.offline {
                break Err(ReadingFailure::Offline.into());
            }
            if !kept {
                kept = true;
                held::keep(store, self.held());
            }
            if setup.wifi && !said {
                said = true;
                let line =
                    Msg::Reading(ReadingMsg::ExportWaitsForWifi { n: i64::from(missing) });
                let chapters: Vec<ChapterId> =
                    wanted.iter().map(|w| w.chapter.clone()).collect();
                for job in self.told.waits(Some((line.clone(), chapters.clone()))) {
                    let _ = self.lent.tx.send(Event::Reading(ReadingEvent::ExportWaiting {
                        job,
                        work: self.work.clone(),
                        target: self.target.clone(),
                        line: line.clone(),
                        chapters: chapters.clone(),
                    }));
                }
            }
            // A change to the network or the setting, or a look at whether it was cancelled.
            let _ = tokio::time::timeout(LOOK_AGAIN, changes.changed()).await;
        };
        if said {
            self.told.waits(None);
        }
        if kept {
            held::forget(store, &self.key);
        }
        result
    }

    /// Keep this ready send beside its files, for a launch after the app stops to offer again.
    fn keep_ready(&self, files: &[kaeri_model::reading::ExportFile]) {
        let sent = self.sent.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if let Some(record) =
            kept::KeptSend::new(&self.work, &self.target, &sent, &self.out, files)
        {
            record.write(&self.out);
        }
    }

    /// This send as the list of those waiting keeps it.
    fn held(&self) -> held::HeldSend {
        held::HeldSend {
            key: self.key.clone(),
            work: self.work.as_str().to_owned(),
            pick: held::Pick::from(&self.pick),
            target: self.target.clone(),
            format: held::Format::from(self.format),
        }
    }
}

/// How often a waiting send looks whether it was cancelled.
const LOOK_AGAIN: Duration = Duration::from_millis(200);

/// The ready sends earlier launches kept, each offered again as `job` of this launch, which
/// `next_job` gives: told as `ExportKept`, kept for its Share and put away by `CancelPrepare`
/// as a ready send of this launch is.
pub(crate) fn offer_kept(
    engine: &EngineState,
    services: &Services,
    mut next_job: impl FnMut() -> JobId,
    tx: &Outbox,
) {
    if !engine.reading.available {
        return;
    }
    let exports = &services.book_sources.exports;
    let now = services.clock.now();
    let wall = kept::wall_now();
    for (name, send) in exports.kept() {
        let job = next_job();
        let files = send.files_in(&exports.dir.join(&name));
        exports.adopt(job, name, now.saturating_sub(wall.saturating_sub(send.at)));
        let _ = tx.send(Event::Reading(ReadingEvent::ExportKept {
            job,
            work: WorkId::new(send.work),
            target: send.target,
            chapters: send.chapters.iter().map(kaeri_core::books::ChapterId::new).collect(),
            files,
        }));
    }
}

/// The sends left waiting when the engine last stopped, asked for again: each starts as a job
/// of its own and waits again while it must.
pub(crate) fn held_sends(engine: &EngineState, services: &Services) -> Vec<ReadingCommand> {
    if !engine.reading.available {
        return Vec::new();
    }
    held::take(services.book_sources.store()).iter().map(held::HeldSend::command).collect()
}

// ---------------------------------------------------------------------------------------------
// The feed

/// Where the feed's password is kept: the secure store the trackers' tokens are in, which is
/// the keychain, the Credential Locker or libsecret, or the `0600` file the token storage
/// setting falls back to. The config holds only whether one is set.
pub const FEED_PASSWORD: &str = "opds-feed";

/// Keeps the feed's password in the secure store, or forgets it when empty, and says whether
/// one is set now. A store that refuses leaves no password.
pub(crate) fn keep_password(services: &Services, password: &str) -> bool {
    let password = password.trim();
    if password.is_empty() {
        if let Err(e) = services.tokens.clear(FEED_PASSWORD) {
            tracing::warn!(error = %e, "the feed password was not forgotten");
        }
        return false;
    }
    match services.tokens.set(FEED_PASSWORD, password) {
        Ok(_) => true,
        Err(e) => {
            tracing::warn!(error = %e, "the feed password was not kept");
            false
        }
    }
}

#[cfg(feature = "opds-feed")]
fn password_of(services: &Services) -> Option<String> {
    services.tokens.get(FEED_PASSWORD).ok().filter(|p| !p.is_empty())
}

#[cfg(feature = "opds-feed")]
fn feed_view(engine: &EngineState, services: &Services) -> FeedView {
    services.book_sources.feed.view(
        password_of(services),
        services.book_sources.store(),
        &engine.config.effective().reading.feed,
    )
}

#[cfg(feature = "opds-feed")]
fn set_feed_share(
    engine: &mut EngineState,
    services: &Services,
    shelf: kaeri_model::reading::FeedShelf,
    on: bool,
    tx: &Outbox,
) {
    use kaeri_core::settings::SettingValue;
    use kaeri_model::reading::FeedShelf;
    fn share(feed: &mut kaeri_core::config::FeedConfig, shelf: FeedShelf) -> &mut Option<bool> {
        match shelf {
            FeedShelf::Downloads => &mut feed.share_downloads,
            FeedShelf::MangaFolders => &mut feed.share_manga,
            FeedShelf::NovelFolders => &mut feed.share_novels,
        }
    }
    let key = match shelf {
        FeedShelf::Downloads => "share_downloads",
        FeedShelf::MangaFolders => "share_manga",
        FeedShelf::NovelFolders => "share_novels",
    };
    // In effect before it is written, as a setting is: the feed reads what is in effect.
    *share(&mut engine.config.effective_mut().reading.feed, shelf) = Some(on);
    if let Err(e) =
        engine.config.write(&services.paths, &["reading", "feed"], key, SettingValue::Bool(on))
    {
        tracing::warn!(error = %e, "the feed shelf was not kept");
        let mut saved = engine.config.saved().reading.feed.clone();
        *share(&mut engine.config.effective_mut().reading.feed, shelf) =
            *share(&mut saved, shelf);
    }
    apply_feed(engine, services, tx);
}

#[cfg(not(feature = "opds-feed"))]
fn set_feed_share(
    _: &mut EngineState,
    _: &Services,
    _: kaeri_model::reading::FeedShelf,
    _: bool,
    tx: &Outbox,
) {
    let _ = tx.send(Event::Reading(ReadingEvent::Feed(FeedView::default())));
}

#[cfg(not(feature = "opds-feed"))]
fn feed_view(_engine: &EngineState, _services: &Services) -> FeedView {
    FeedView::default()
}

#[cfg(feature = "opds-feed")]
fn set_feed(
    engine: &mut EngineState,
    services: &Services,
    on: bool,
    password: Option<String>,
    tx: &Outbox,
) {
    use kaeri_core::settings::SettingValue;
    let paths = &services.paths;
    // Each value is put in effect before it is written, as a setting is: `write` keeps the
    // file and puts back only a value it refused, and the feed follows what is in effect. So
    // off stops serving at once, and on starts at once.
    if let Some(password) = password {
        let set = keep_password(services, &password);
        engine.config.effective_mut().reading.feed.password_set = set;
        if let Err(e) = engine.config.write(
            paths,
            &["reading", "feed"],
            "password_set",
            SettingValue::Bool(set),
        ) {
            tracing::warn!(error = %e, "the feed password flag was not kept");
        }
    }
    engine.config.effective_mut().reading.feed.on = on;
    if let Err(e) =
        engine.config.write(paths, &["reading", "feed"], "on", SettingValue::Bool(on))
    {
        tracing::warn!(error = %e, "the feed switch was not kept");
    }
    // Off stays off for this launch even when the file refused it: the library is not served
    // a moment longer than the person wants.
    if !on {
        engine.config.effective_mut().reading.feed.on = false;
    }
    apply_feed(engine, services, tx);
}

#[cfg(not(feature = "opds-feed"))]
fn set_feed(_: &mut EngineState, _: &Services, _: bool, _: Option<String>, tx: &Outbox) {
    // Not in this build: the apps without the feed never send it.
    let _ = tx.send(Event::Reading(ReadingEvent::Feed(FeedView::default())));
}

/// The feed follows the config: started when it is on and has a password, stopped when not.
/// Called when settings are saved and at launch.
#[cfg(feature = "opds-feed")]
pub(crate) fn apply_feed(engine: &EngineState, services: &Services, tx: &Outbox) {
    if !engine.reading.available {
        return;
    }
    let config = engine.config.effective().reading.feed.clone();
    services.book_sources.feed.apply(
        &config,
        password_of(services),
        services.book_sources.store().clone(),
        tx,
    );
}

#[cfg(not(feature = "opds-feed"))]
pub(crate) fn apply_feed(_: &EngineState, _: &Services, _: &Outbox) {}

#[cfg(test)]
mod ready_tests {
    use super::*;

    const NOW: i64 = 1_800_000_000;

    /// A send that is ready, with one file written.
    fn ready_send(exports: &Exports, job: u64, at: i64) -> PathBuf {
        let job = JobId::new(job);
        exports.begin(job);
        let folder = exports.folder(job, at);
        std::fs::create_dir_all(&folder).unwrap();
        let file = folder.join("Made-up Series 1.cbz");
        std::fs::write(&file, b"pk").unwrap();
        exports.ready(job, at);
        exports.end(job);
        file
    }

    /// R5A-6. A send left ready keeps its Share (the send banner and the sheet keep a ready
    /// send until it is shared or put away), so starting another send keeps its files.
    #[test]
    fn r5a_a_ready_send_keeps_its_files_when_another_starts() {
        let dir = tempfile::tempdir().unwrap();
        let exports = Exports::new(dir.path().to_owned());
        let first = ready_send(&exports, 1, NOW);
        let _second = exports.folder(JobId::new(2), NOW + 60);
        assert!(first.exists(), "the ready send's file is gone");
    }

    #[test]
    fn ready_files_go_after_a_day_and_leftovers_at_once() {
        let dir = tempfile::tempdir().unwrap();
        let exports = Exports::new(dir.path().to_owned());
        let leftover = dir.path().join("7");
        std::fs::create_dir_all(dir.path().join("8-fetch")).unwrap();
        std::fs::create_dir_all(&leftover).unwrap();
        let first = ready_send(&exports, 1, NOW);
        assert!(!leftover.exists() && !dir.path().join("8-fetch").exists());
        exports.folder(JobId::new(2), NOW + READY_KEPT - 1);
        assert!(first.exists(), "a ready send younger than a day lost its file");
        exports.folder(JobId::new(3), NOW + READY_KEPT);
        assert!(!first.exists(), "a ready send a day old kept its file");
    }

    /// R6E-5. Job ids start over at each launch: a send of a new launch must never take the
    /// folders a killed send of an earlier one left under the same job id.
    #[test]
    fn a_send_never_takes_the_folders_an_earlier_launch_left_under_its_job_id() {
        let dir = tempfile::tempdir().unwrap();
        let before = Exports::new(dir.path().to_owned());
        let job = JobId::new(1);
        before.begin(job);
        let old_out = before.folder(job, NOW);
        let old_scratch = before.scratch(job);
        std::fs::create_dir_all(old_scratch.join("3.pages")).unwrap();
        std::fs::write(old_scratch.join("3.pages").join("count"), "13").unwrap();
        // Killed here: the next launch is a new engine, whose first send is job 1 again.
        let after = Exports::new(dir.path().to_owned());
        after.begin(job);
        let new_out = after.folder(job, NOW + 60);
        let new_scratch = after.scratch(job);
        assert_ne!(new_out, old_out);
        assert_ne!(new_scratch, old_scratch);
        assert!(!new_scratch.exists(), "the new send starts with no pages");
        assert!(!old_scratch.exists(), "the dead send's pages were left");
    }

    #[test]
    fn putting_a_ready_send_away_removes_its_files() {
        let dir = tempfile::tempdir().unwrap();
        let exports = Exports::new(dir.path().to_owned());
        let first = ready_send(&exports, 1, NOW);
        assert!(!exports.cancel(JobId::new(1)));
        assert!(!first.exists());
        // A running send is stopped, not removed: its own end cleans up.
        let running = JobId::new(2);
        let flag = exports.begin(running);
        assert!(exports.cancel(running) && flag.load(Ordering::Relaxed));
    }

    /// Android R7-2. A ready send kept beside its files outlives the process that made it: the
    /// launch sweep leaves it while it is kept, the next engine offers it once, a new send
    /// keeps it, and putting it away removes it. One past a day, or never kept, goes.
    #[test]
    fn a_ready_send_kept_beside_its_files_outlives_the_process() {
        use kaeri_core::books::ChapterId;
        use kaeri_model::reading::ExportFile;
        let dir = tempfile::tempdir().unwrap();
        let wall = kept::wall_now();
        let book = "Made-up Series 1.kepub.epub";
        // As a process that has ended left them; no process has the id 0.
        let folder = |name: &str| {
            let folder = dir.path().join(name);
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(folder.join(book), b"pk").unwrap();
            folder
        };
        let keep = |folder: &Path, at: i64| {
            let path = folder.join(book).display().to_string();
            let file = ExportFile { path, name: book.into(), bytes: 2, part: None };
            let work = WorkId::new("a:900601");
            let chapters = [ChapterId::new("1")];
            let mut send =
                kept::KeptSend::new(&work, "kobo", &chapters, folder, &[file]).unwrap();
            send.at = at;
            send.write(folder);
        };
        let live = folder("0.dead-1");
        keep(&live, wall - 60);
        let old = folder("0.dead-2");
        keep(&old, wall - READY_KEPT - 1);
        let bare = folder("0.dead-3");
        assert_eq!(sweep(dir.path()), 2);
        assert!(live.exists() && !old.exists() && !bare.exists());

        let exports = Exports::new(dir.path().to_owned());
        let found = exports.kept();
        assert_eq!(found.len(), 1);
        let (name, send) = found[0].clone();
        assert_eq!((name.as_str(), send.target.as_str()), ("0.dead-1", "kobo"));
        assert_eq!(send.chapters, ["1"]);
        assert_eq!(send.files_in(&live)[0].path, live.join(book).display().to_string());
        exports.adopt(JobId::new(4), name, NOW);
        assert!(exports.kept().is_empty(), "offered once");
        exports.folder(JobId::new(5), NOW + 60);
        assert!(live.join(book).exists(), "a new send keeps it");
        assert!(!exports.cancel(JobId::new(4)));
        assert!(!live.exists(), "put away");
    }
}
