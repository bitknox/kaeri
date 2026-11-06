//! Send to e-reader and the OPDS feed: the engine's side of getting what is on this device
//! onto another reader.
//!
//! **Send to e-reader** prepares downloaded chapters for one target profile and hands back a
//! list of files for the app's share sheet: [`prepare`] writes them, [`profiles`] says what a
//! target is. It works from downloads, so a chapter that is not downloaded is not sent. The job
//! is announced like any other, told as `ExportProgress` and ended with `ExportReady`; it stops
//! on `CancelPrepare` and leaves no files behind. Once the files are shared, the series page asks
//! where you stopped, and the answer is a `ReadAdopt` op.
//!
//! **The OPDS feed** is in [`feed`], behind the `opds-feed` feature the desktop builds turn on.

pub mod prepare;
pub mod profiles;

#[cfg(feature = "opds-feed")]
pub mod feed;

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use kaeri_core::books::WorkId;
use kaeri_model::{
    event::Event,
    jobs::JobId,
    reading::{
        DownloadPick, ExportFormat, FeedView, ReadingAbout, ReadingCommand, ReadingEvent,
        ReadingFailure, VolumeGroup,
    },
};
use kaeri_store::{Store, book_downloads::BookDownloadState};

use crate::{
    downloads::books::{BookDownloads, chapter_row, flatten},
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
                if !busy.contains(&name) {
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
            | ReadingCommand::LoadFeed
            | ReadingCommand::SetFeed { .. }
    )
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
            let targets = profiles::all(&engine.config.effective().reading)
                .iter()
                .map(profiles::Profile::target)
                .collect();
            let _ = tx.send(Event::Reading(ReadingEvent::ExportTargets(targets)));
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
    let chapters = chosen(store, work.as_str(), &pick);
    if chapters.is_empty() {
        // Nothing downloaded to send.
        return fail(about, ReadingFailure::NotFound, tx);
    }
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
    let first = chapters.first().map(|c| c.label.clone()).unwrap_or_default();
    let last = chapters.last().map(|c| c.label.clone()).unwrap_or_default();
    let request = prepare::Request {
        identifier: format!("urn:kaeri:{}:{first}-{last}", work.as_str()),
        title,
        chapters,
        profile,
        format,
        out: exports.folder(id),
        limit: None,
        right_to_left,
    };
    let tx = tx.clone();
    tokio::task::spawn_blocking(move || {
        let _job = job;
        let mut told = Instant::now() - Duration::from_secs(1);
        let mut progress = |done: u32, total: u32| {
            if done < total && told.elapsed() < Duration::from_millis(150) {
                return;
            }
            told = Instant::now();
            let _ =
                tx.send(Event::Reading(ReadingEvent::ExportProgress { job: id, done, total }));
        };
        let result = prepare::run(&request, &mut progress, &cancel);
        exports.end(id);
        match result {
            Ok(files) => {
                let _ = tx.send(Event::Reading(ReadingEvent::ExportReady { job: id, files }));
            }
            Err(prepare::Stop::Cancelled) => {}
            Err(prepare::Stop::Failed(failure)) => fail(about, failure, &tx),
        }
    });
}

// ---------------------------------------------------------------------------------------------
// The feed

#[cfg(feature = "opds-feed")]
fn feed_view(engine: &EngineState, services: &Services) -> FeedView {
    services
        .book_sources
        .feed
        .view(&engine.config.effective().reading.feed, services.book_sources.store())
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
    if let Some(password) = password
        && let Err(e) = engine.config.write(
            paths,
            &["reading", "feed"],
            "password",
            SettingValue::Str(password),
        )
    {
        tracing::warn!(error = %e, "the feed password was not kept");
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
    services.book_sources.feed.apply(&config, services.book_sources.store().clone(), tx);
}

#[cfg(not(feature = "opds-feed"))]
pub(crate) fn apply_feed(_: &EngineState, _: &Services, _: &Outbox) {}
