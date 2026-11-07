//! Send to e-reader and the OPDS feed: the engine's side of getting what is on this device
//! onto another reader.
//!
//! **Send to e-reader** prepares chapters for one target profile and hands back a
//! list of files for the app's share sheet: [`prepare`] writes them, [`profiles`] says what a
//! target is. A chapter that is not downloaded is fetched through the download pipeline for the
//! send and removed once its files are written; downloaded ones are used as they are. The job
//! is announced like any other, told as `ExportProgress` and ended with `ExportReady`; it stops
//! on `CancelPrepare` and leaves no files behind. A send with chapters to fetch waits while the
//! downloads do, and goes on by itself: for Wi-Fi it says so with `ExportWaiting`, and one left
//! waiting when the engine stops starts again at the next launch ([`held`]). Until the front end
//! first says the network it waits too, without a word, as the network may be cellular. With no
//! network at all it fails as offline. Once the files are shared, the series page asks
//! where you stopped, and the answer is a `ReadAdopt` op.
//!
//! **The OPDS feed** is in [`feed`], behind the `opds-feed` feature the desktop builds turn on.

pub(crate) mod held;
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

use kaeri_core::books::WorkId;
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
        BookDownloads, Fetching, Lent, Stop as FetchStop, chapter_row, flatten,
    },
    jobs::JobGuard,
    outbox::Outbox,
    services::Services,
    state::EngineState,
};

/// The sends under way, so one can be stopped, and where their files go.
#[derive(Clone, Debug)]
pub struct Exports {
    dir: PathBuf,
    running: Arc<Mutex<HashMap<u64, Arc<AtomicBool>>>>,
}

impl Exports {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir, running: Arc::default() }
    }

    fn begin(&self, job: JobId) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        self.running.lock().unwrap_or_else(|e| e.into_inner()).insert(job.get(), flag.clone());
        flag
    }

    fn end(&self, job: JobId) {
        self.running.lock().unwrap_or_else(|e| e.into_inner()).remove(&job.get());
    }

    /// Stops a send; false when it is not running.
    pub fn cancel(&self, job: JobId) -> bool {
        let running = self.running.lock().unwrap_or_else(|e| e.into_inner());
        match running.get(&job.get()) {
            Some(flag) => {
                flag.store(true, Ordering::Relaxed);
                true
            }
            None => false,
        }
    }

    /// The folder one send writes to, with the files of earlier sends removed: they were
    /// shared, or abandoned, by the time another is asked for.
    fn folder(&self, job: JobId) -> PathBuf {
        let busy: Vec<String> = self
            .running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .keys()
            .map(|k| k.to_string())
            .collect();
        if let Ok(entries) = std::fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                // A send's scratch folder is its id and `-fetch`.
                let running = busy
                    .iter()
                    .any(|b| name == *b || name.strip_suffix("-fetch") == Some(b.as_str()));
                if !running {
                    let _ = std::fs::remove_dir_all(entry.path());
                }
            }
        }
        self.dir.join(job.get().to_string())
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
            if let Err(e) = engine.config.write(
                &services.paths,
                &["reading"],
                "send_target",
                SettingValue::Str(target),
            ) {
                tracing::warn!(error = %e, "the send target was not kept");
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

/// The chapters the person has downloaded, by chapter id, with the file each is.
fn chosen_downloads(store: &Store, work: &str) -> HashMap<String, PathBuf> {
    store
        .book_downloads(Some(work))
        .unwrap_or_default()
        .into_iter()
        .filter(|d| d.state == BookDownloadState::Done)
        .filter_map(|d| {
            let path = PathBuf::from(d.path?);
            path.is_file().then_some((d.chapter, path))
        })
        .collect()
}

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
    let rows =
        flatten(vec![VolumeGroup { volume: None, title: None, chapters, all_read: false }]);
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
    let title = store
        .book_work_kind(work.as_str())
        .ok()
        .flatten()
        .and_then(|kind| kind.title)
        .unwrap_or_else(|| work.as_str().to_owned());
    let right_to_left = store
        .series_settings(work.as_str())
        .unwrap_or_default()
        .into_iter()
        .find(|s| s.setting == "direction")
        .map(|s| s.value == "rtl");
    let id = job.as_ref().map_or(JobId::new(0), |j| j.id);
    let exports = services.book_sources.exports.clone();
    let cancel = exports.begin(id);
    let out = exports.folder(id);
    let send = OneSend {
        key: id.get().to_string(),
        target: target.to_owned(),
        lent: Lent::of(services, tx),
        downloads: services.book_sources.downloads.clone(),
        exports,
        told: Told::new(tx.clone(), id),
        cancel,
        id,
        work,
        pick,
        title,
        profile,
        format,
        out,
        right_to_left,
    };
    tokio::spawn(async move {
        let _job = job;
        let tx = send.lent.tx.clone();
        let result = send.run().await;
        send.exports.end(send.id);
        let chapter = send.told.failing_chapter();
        match result {
            Ok(files) => {
                let _ =
                    tx.send(Event::Reading(ReadingEvent::ExportReady { job: send.id, files }));
            }
            Err(prepare::Stop::Cancelled) => {}
            Err(prepare::Stop::Failed(failure)) => {
                let _ = tx.send(Event::Reading(ReadingEvent::ExportFailed {
                    job: send.id,
                    chapter,
                    failure,
                }));
            }
        }
    });
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
}

impl Told {
    fn new(tx: Outbox, job: JobId) -> Arc<Self> {
        Arc::new(Self {
            tx,
            job,
            last: Mutex::new(Instant::now() - Duration::from_secs(1)),
            gathered: AtomicU32::new(0),
            of: AtomicU32::new(0),
            current: Mutex::new(None),
        })
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
        let _ = self.tx.send(Event::Reading(ReadingEvent::ExportProgress {
            job: self.job,
            done,
            total,
            chapters_done: self.gathered.load(Ordering::Relaxed),
            chapters: self.of.load(Ordering::Relaxed),
        }));
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
    right_to_left: Option<bool>,
}

impl OneSend {
    async fn run(&self) -> Result<Vec<kaeri_model::reading::ExportFile>, prepare::Stop> {
        let temp = self.exports.dir.join(format!("{}-fetch", self.id.get()));
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
            right_to_left: self.right_to_left,
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
        let downloaded = chosen_downloads(store, self.work.as_str());
        let asked_all_downloaded = match &self.pick {
            DownloadPick::Chapters { ids } => {
                !ids.is_empty() && ids.iter().all(|c| downloaded.contains_key(c.as_str()))
            }
            _ => false,
        };
        if asked_all_downloaded {
            // Nothing to fetch, so nothing to ask a source.
            let chapters = chosen(store, self.work.as_str(), &self.pick);
            return Ok((chapters, 0));
        }
        let now = self.lent.clock.now();
        self.lent.sources.adopt_plugins(&self.lent.plugins);
        let (source, rows) =
            crate::downloads::books::ordered_chapters(&self.lent.sources, &self.work, now)
                .await
                .map_err(prepare::Stop::Failed)?;
        let wanted = BookDownloads::picked(&self.pick, &rows);
        if wanted.is_empty() {
            return Err(ReadingFailure::NotFound.into());
        }
        let to_fetch =
            wanted.iter().filter(|w| !downloaded.contains_key(w.chapter.as_str())).count();
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
            if let Some(path) = downloaded.get(want.chapter.as_str()) {
                chapters.push(prepare::Chapter { label, path: path.clone() });
                self.told.gathered.fetch_add(1, Ordering::Relaxed);
                continue;
            }
            self.told.chapter(Some(label.clone()));
            let missing = u32::try_from(to_fetch).unwrap_or(u32::MAX).saturating_sub(fetched);
            self.may_fetch(missing).await?;
            std::fs::create_dir_all(temp).map_err(|_| {
                prepare::Stop::Failed(ReadingFailure::FileMissing {
                    path: temp.display().to_string(),
                })
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
                source_id: source.clone(),
                stem: temp.join(crate::downloads::books::safe(want.chapter.as_str())),
                wanted: &wanted_now,
                progress: &progress,
            };
            let file = loop {
                match self.downloads.fetch_with(&self.lent, &job).await {
                    Ok(file) => break file,
                    Err(FetchStop::Cancelled) => return Err(prepare::Stop::Cancelled),
                    // The network changed under the fetch: wait, then fetch the chapter again.
                    Err(FetchStop::Held) => self.may_fetch(missing).await?,
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

    /// Wait until chapters may be fetched, `missing` of them still to come. A send waits while
    /// the downloads do: for Wi-Fi it says so, and it is kept in the store meanwhile. With no
    /// network at all it fails as offline; cancelled, it stops.
    async fn may_fetch(&self, missing: u32) -> Result<(), prepare::Stop> {
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
                let _ = self.lent.tx.send(Event::Reading(ReadingEvent::ExportWaiting {
                    job: self.id,
                    work: self.work.clone(),
                    target: self.target.clone(),
                    line,
                }));
            }
            // A change to the network or the setting, or a look at whether it was cancelled.
            let _ = tokio::time::timeout(LOOK_AGAIN, changes.changed()).await;
        };
        if kept {
            held::forget(store, &self.key);
        }
        result
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
    let key = match shelf {
        FeedShelf::Downloads => "share_downloads",
        FeedShelf::MangaFolders => "share_manga",
        FeedShelf::NovelFolders => "share_novels",
    };
    if let Err(e) =
        engine.config.write(&services.paths, &["reading", "feed"], key, SettingValue::Bool(on))
    {
        tracing::warn!(error = %e, "the feed shelf was not kept");
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
    if let Some(password) = password {
        let set = keep_password(services, &password);
        if let Err(e) = engine.config.write(
            paths,
            &["reading", "feed"],
            "password_set",
            SettingValue::Bool(set),
        ) {
            tracing::warn!(error = %e, "the feed password flag was not kept");
        }
    }
    if let Err(e) =
        engine.config.write(paths, &["reading", "feed"], "on", SettingValue::Bool(on))
    {
        tracing::warn!(error = %e, "the feed switch was not kept");
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
