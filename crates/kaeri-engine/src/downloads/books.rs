//! Chapter downloads: the reading queue beside the torrent queue.
//!
//! A chapter is fetched through the book source it was listed by, the way the reader fetches
//! pages, and written whole into the app's data folder: one CBZ for a chapter of page images,
//! a single-chapter EPUB for a text chapter, and a book file a source hands over as it is. The
//! reader finds finished ones through the shelf ([`kaeri_store::Store::shelf`]), so a downloaded
//! chapter reads offline with no part of the reader knowing the difference.
//!
//! **One chapter at a time.** The queue drains oldest first, chapters asked for by hand before
//! those kept ready, with a short rest between chapters so a long queue is paced rather than a
//! burst. A fetch is written under a temporary name and renamed, so a path the shelf gives out
//! always holds a whole file.
//!
//! **Wi-Fi only until told otherwise.** On cellular, in Low Data Mode, offline, or with the app
//! off screen (but for a downloads window) the queue holds; rows show as waiting for Wi-Fi
//! where that is why, and go on by themselves when the network or the setting changes.
//!
//! **Keeping chapters ready.** With the setting on (it is on in the e-reader build), starting
//! or finishing a chapter queues the next [`KEEP_AHEAD`] unread ones after it. They are marked
//! as kept ready, and reading past one lets it go. A chapter let go of by hand is not fetched
//! again on its own. **Delete after reading** lets go of every read chapter's download, never a
//! bookmarked one. Both run when a chapter counts as read, here or on another device.

use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use kaeri_core::{
    books::{ChapterContent, ChapterId, WorkId},
    config::Config,
};
use kaeri_model::{
    command::AppState,
    event::Event,
    medium::AppEdition,
    reading::{
        BookDownloadRow, BookDownloadsView, ChapterDownloadState, ChapterReadState, ChapterRow,
        DownloadPick, ReadingEvent, ReadingFailure, StorageMeter, VolumeGroup,
    },
};
use kaeri_store::{
    Store,
    book_downloads::{BookDownload, BookDownloadAsk, BookDownloadState, Queued},
};

use crate::{
    book_plugins::BookPlugins, clock::Clock, outbox::Outbox, phases::Network,
    read_sources::BookSources, services::Services,
};

/// How many unread chapters after the one being read are kept ready.
pub const KEEP_AHEAD: usize = 3;
/// What a device keeps free of downloads: below this the queue stops with
/// [`ReadingFailure::DiskFull`].
pub const KEEP_FREE: u64 = 512 * 1_000_000;
/// Progress is told at most this often; a chapter starting, finishing or failing is told at once.
const PROGRESS_EVERY: Duration = Duration::from_secs(2);
/// The rest between chapters.
const PACE: Duration =
    if cfg!(test) { Duration::from_millis(2) } else { Duration::from_millis(400) };

/// What the engine lends a download: the sources, the clock and the way to speak.
#[derive(Clone)]
pub struct Lent {
    pub sources: BookSources,
    pub plugins: BookPlugins,
    pub clock: Clock,
    pub tx: Outbox,
}

impl Lent {
    pub fn of(services: &Services, tx: &Outbox) -> Self {
        Self {
            sources: services.book_sources.clone(),
            plugins: services.book_plugins.clone(),
            clock: services.clock.clone(),
            tx: tx.clone(),
        }
    }
}

/// What decides whether the queue runs, and what it does with a chapter once read. Set by the
/// engine from the config, the network and where the app is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Setup {
    /// Nothing is fetched now.
    pub hold: bool,
    /// The hold is for Wi-Fi: rows show it.
    pub wifi: bool,
    /// There is no network at all.
    pub offline: bool,
    pub keep_ready: bool,
    pub delete_after: bool,
}

impl Setup {
    pub fn of(phase: AppState, network: Network, config: &Config, edition: AppEdition) -> Self {
        let reading = &config.reading;
        let wifi = network.constrained || (network.cellular && !reading.downloads_on_cellular);
        let away = matches!(phase, AppState::Background { .. });
        Self {
            hold: wifi || network.offline || away,
            wifi,
            offline: network.offline,
            keep_ready: reading
                .keep_chapters_ready
                .unwrap_or(matches!(edition, AppEdition::ReadOnly)),
            delete_after: reading.delete_after_reading,
        }
    }
}

/// The reading download queue. Cheap to clone.
#[derive(Clone)]
pub struct BookDownloads(Arc<Inner>);

struct Inner {
    dir: PathBuf,
    pace: Duration,
    running: Mutex<bool>,
    setup: Mutex<Setup>,
    /// Told each time the setup changes, for a send that waits on it.
    changed: tokio::sync::watch::Sender<Setup>,
    /// What a test takes to be free on the device, in place of asking the disk.
    free: Mutex<Option<u64>>,
    told: Mutex<Option<Instant>>,
}

impl std::fmt::Debug for BookDownloads {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BookDownloads").field("dir", &self.0.dir).finish()
    }
}

/// A chapter to queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanted {
    pub chapter: ChapterId,
    pub version_label: Option<String>,
}

/// Why a fetch ended without a file.
pub(crate) enum Stop {
    Failed(ReadingFailure),
    /// The person took it out of the queue.
    Cancelled,
    /// The network or the setting changed under it: it goes back in the queue.
    Held,
}

impl From<ReadingFailure> for Stop {
    fn from(failure: ReadingFailure) -> Self {
        Self::Failed(failure)
    }
}

impl BookDownloads {
    /// Downloads kept under `dir`.
    pub fn new(dir: PathBuf) -> Self {
        Self::with_pace(dir, PACE)
    }

    fn with_pace(dir: PathBuf, pace: Duration) -> Self {
        Self(Arc::new(Inner {
            dir,
            pace,
            running: Mutex::new(false),
            setup: Mutex::new(Setup::default()),
            changed: tokio::sync::watch::channel(Setup::default()).0,
            free: Mutex::new(None),
            told: Mutex::new(None),
        }))
    }

    pub fn dir(&self) -> &Path {
        &self.0.dir
    }

    pub fn setup(&self) -> Setup {
        *self.0.setup.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Take in the config, the network and the phase. Says whether the queue may run now.
    pub fn configure(&self, setup: Setup) -> bool {
        *self.0.setup.lock().unwrap_or_else(|e| e.into_inner()) = setup;
        self.0.changed.send_if_modified(|was| std::mem::replace(was, setup) != setup);
        !setup.hold
    }

    /// Changes to the setup, as [`Self::configure`] takes them in.
    pub(crate) fn changes(&self) -> tokio::sync::watch::Receiver<Setup> {
        self.0.changed.subscribe()
    }

    /// Fetches the last run left half done go back in the queue, and half-written files go.
    pub fn recover(&self, store: &Store, now: i64) {
        match store.book_downloads_recover(now) {
            Ok(0) => {}
            Ok(n) => tracing::info!(n, "chapter downloads cut short are queued again"),
            Err(e) => tracing::warn!(error = %e, "downloads cut short were not queued again"),
        }
        let Ok(works) = std::fs::read_dir(&self.0.dir) else { return };
        for work in works.flatten() {
            let Ok(files) = std::fs::read_dir(work.path()) else { continue };
            for file in files.flatten() {
                if file.path().extension().is_some_and(|e| e == "part") {
                    let _ = std::fs::remove_file(file.path());
                }
            }
        }
    }

    fn free(&self) -> Option<u64> {
        let pretend = *self.0.free.lock().unwrap_or_else(|e| e.into_inner());
        pretend.or_else(|| crate::downloads::ahead::free_space(&self.0.dir))
    }

    // -----------------------------------------------------------------------------------------
    // What is shown

    /// How each of a work's chapters stands, by chapter.
    pub fn states(&self, store: &Store, work: &str) -> HashMap<String, ChapterDownloadState> {
        let wifi = self.setup().wifi;
        store
            .book_downloads(Some(work))
            .unwrap_or_default()
            .iter()
            .map(|d| (d.chapter.clone(), state_of(d, wifi)))
            .collect()
    }

    /// Every download with how it stands, and what they take up.
    pub fn view(&self, store: &Store) -> BookDownloadsView {
        let wifi = self.setup().wifi;
        let rows = store.book_downloads(None).unwrap_or_default();
        let mut marks: HashMap<String, kaeri_store::reading::ReadMarks> = HashMap::new();
        let mut shown = Vec::with_capacity(rows.len());
        for d in &rows {
            let read = marks
                .entry(d.work.clone())
                .or_insert_with(|| store.read_marks(&d.work).unwrap_or_default())
                .is_read(&d.chapter);
            let bookmarked = store.bookmarks(&d.work).unwrap_or_default().contains(&d.chapter);
            let title = store
                .book_work_kind(&d.work)
                .ok()
                .flatten()
                .and_then(|kind| kind.title)
                .unwrap_or_else(|| d.work.clone());
            shown.push(BookDownloadRow {
                work: WorkId::new(d.work.clone()),
                title,
                chapter: chapter_row(d, read, bookmarked),
                state: state_of(d, wifi),
            });
        }
        let (used, chapters) = store.book_download_totals().unwrap_or((0, 0));
        let free = self.free().unwrap_or(0);
        BookDownloadsView { rows: shown, meter: StorageMeter { used, free, chapters } }
    }

    /// Tell the screens how downloads stand.
    pub fn publish(&self, store: &Store, tx: &Outbox) {
        let _ = tx.send(Event::Reading(ReadingEvent::Downloads(self.view(store))));
    }

    fn publish_progress(&self, store: &Store, tx: &Outbox) {
        let mut told = self.0.told.lock().unwrap_or_else(|e| e.into_inner());
        if told.is_some_and(|at| at.elapsed() < PROGRESS_EVERY) {
            return;
        }
        *told = Some(Instant::now());
        drop(told);
        self.publish(store, tx);
    }

    // -----------------------------------------------------------------------------------------
    // Asking

    /// Queue chapters of a work from `source`, kept ready or asked for. Says how many were
    /// newly queued.
    pub fn queue(
        &self,
        store: &Store,
        work: &WorkId,
        source: &str,
        wanted: &[Wanted],
        keep_ready: bool,
        now: i64,
    ) -> usize {
        let mut added = 0;
        for want in wanted {
            let ask = BookDownloadAsk {
                work: work.as_str(),
                chapter: want.chapter.as_str(),
                source_id: source,
                version_label: want.version_label.as_deref(),
                keep_ready,
            };
            match store.queue_book_download(&ask, now) {
                Ok(Queued::Added) => added += 1,
                Ok(Queued::Had | Queued::Skipped) => {}
                Err(e) => tracing::warn!(error = %e, "a chapter was not queued"),
            }
        }
        added
    }

    /// The chapters `pick` names, in reading order, out of a work's list.
    pub fn picked(pick: &DownloadPick, rows: &[ChapterRow]) -> Vec<Wanted> {
        let unread = |r: &&ChapterRow| !matches!(r.state, ChapterReadState::Read);
        let chosen: Vec<&ChapterRow> = match pick {
            DownloadPick::NextUnread { n } => {
                rows.iter().filter(unread).take(*n as usize).collect()
            }
            DownloadPick::Unread => rows.iter().filter(unread).collect(),
            DownloadPick::Volume { volume } => {
                rows.iter().filter(|r| r.volume.as_deref() == Some(volume.as_str())).collect()
            }
            DownloadPick::Chapters { ids } => {
                rows.iter().filter(|r| ids.contains(&r.id)).collect()
            }
        };
        chosen
            .into_iter()
            .map(|r| Wanted { chapter: r.id.clone(), version_label: r.version_label.clone() })
            .collect()
    }

    /// Ask for the chapters `pick` names. Lists the work's chapters first, so this answers
    /// after the source does; a source that cannot be asked ends in `Failed` for the screen
    /// that asked.
    pub async fn download(&self, lent: &Lent, work: WorkId, pick: DownloadPick) {
        let now = lent.clock.now();
        lent.sources.adopt_plugins(&lent.plugins);
        let listed = ordered_chapters(&lent.sources, &work, now).await;
        let (source, rows) = match listed {
            Ok(found) => found,
            Err(failure) => {
                let about = kaeri_model::reading::ReadingAbout::Downloads;
                let _ = lent.tx.send(Event::Reading(ReadingEvent::Failed { about, failure }));
                return;
            }
        };
        let wanted = Self::picked(&pick, &rows);
        let store = lent.sources.store();
        if self.queue(store, &work, &source, &wanted, false, now) > 0 || !wanted.is_empty() {
            self.publish(store, &lent.tx);
            self.start(lent);
        }
    }

    /// Queue every unread chapter Updates lists.
    pub(crate) async fn download_new(
        &self,
        kit: &crate::read_catalog::Kit,
        ask: crate::read_catalog::Ask,
        lent: &Lent,
    ) {
        let view = crate::read_catalog::updates::view(kit, ask);
        let store = lent.sources.store();
        let mut by_work: Vec<(WorkId, Vec<Wanted>)> = Vec::new();
        for row in view.days.iter().flat_map(|d| &d.rows) {
            if matches!(row.chapter.state, ChapterReadState::Read) {
                continue;
            }
            let want = Wanted {
                chapter: row.chapter.id.clone(),
                version_label: row.chapter.version_label.clone(),
            };
            match by_work.iter_mut().find(|(w, _)| *w == row.tile.work) {
                Some((_, list)) => list.push(want),
                None => by_work.push((row.tile.work.clone(), vec![want])),
            }
        }
        let now = lent.clock.now();
        let mut added = 0;
        for (work, wanted) in by_work {
            let Some((source, _)) = lent.sources.edition_of(&work, None) else { continue };
            added += self.queue(store, &work, &source, &wanted, false, now);
        }
        if added > 0 {
            self.publish(store, &lent.tx);
            self.start(lent);
        }
    }

    /// Cancel what is queued or fetching of `chapters` (all of the work's when empty). Finished
    /// downloads stay.
    pub fn cancel(&self, lent: &Lent, work: &WorkId, chapters: &[ChapterId]) {
        let store = lent.sources.store();
        let ids: Vec<String> = chapters.iter().map(|c| c.as_str().to_owned()).collect();
        let now = lent.clock.now();
        let mut gone = false;
        for row in store.book_downloads(Some(work.as_str())).unwrap_or_default() {
            let waiting =
                matches!(row.state, BookDownloadState::Queued | BookDownloadState::Failed)
                    || row.state == BookDownloadState::Downloading;
            if waiting && (ids.is_empty() || ids.contains(&row.chapter)) {
                let _ = store.remove_book_downloads(work.as_str(), &[row.chapter], false, now);
                gone = true;
            }
        }
        if gone {
            self.publish(store, &lent.tx);
        }
    }

    /// Delete downloaded chapters and their files (all of the work's when `chapters` is empty).
    /// Anything still queued or fetching goes with them.
    pub fn delete(&self, lent: &Lent, work: &WorkId, chapters: &[ChapterId]) -> usize {
        let store = lent.sources.store();
        let ids: Vec<String> = chapters.iter().map(|c| c.as_str().to_owned()).collect();
        let removed = self.remove(store, work.as_str(), &ids, false, lent.clock.now());
        self.publish(store, &lent.tx);
        self.relist(lent, work.clone());
        removed
    }

    /// Delete every download of the works of one medium (`manga` or `novel`), as when the
    /// person turns reading off and asks for them to go.
    pub fn delete_all(&self, lent: &Lent) -> usize {
        let store = lent.sources.store();
        let now = lent.clock.now();
        let mut removed = 0;
        for work in store.book_download_works().unwrap_or_default() {
            removed += self.remove(store, &work, &[], false, now);
        }
        self.publish(store, &lent.tx);
        removed
    }

    fn remove(
        &self,
        store: &Store,
        work: &str,
        chapters: &[String],
        only_done: bool,
        now: i64,
    ) -> usize {
        match store.remove_book_downloads(work, chapters, only_done, now) {
            Ok(rows) => {
                for row in &rows {
                    forget_file(row);
                }
                rows.len()
            }
            Err(e) => {
                tracing::warn!(error = %e, "downloads were not removed");
                0
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // Reading

    /// The reader opened or finished a chapter: keep the next ones ready, and let go of what is
    /// read. Off the caller's thread.
    pub fn read_moved(&self, lent: &Lent, work: &WorkId, chapter: &ChapterId, finished: bool) {
        if finished {
            self.sweep(lent, Some(work.as_str()));
        }
        if !self.setup().keep_ready {
            return;
        }
        let (this, lent, work, chapter) =
            (self.clone(), lent.clone(), work.clone(), chapter.clone());
        tokio::spawn(async move { this.keep_ready(&lent, &work, &chapter).await });
    }

    /// Let go of the downloads of read chapters, of one work or all: every one kept ready on
    /// its own, and every one with delete after reading on, bookmarked chapters spared.
    pub fn sweep(&self, lent: &Lent, work: Option<&str>) -> usize {
        let store = lent.sources.store();
        let delete_after = self.setup().delete_after;
        let works = match work {
            Some(work) => vec![work.to_owned()],
            None => store.book_download_works().unwrap_or_default(),
        };
        let mut gone = 0;
        for work in works {
            let marks = store.read_marks(&work).unwrap_or_default();
            let bookmarks: HashSet<String> =
                store.bookmarks(&work).unwrap_or_default().into_iter().collect();
            for row in store.book_downloads(Some(&work)).unwrap_or_default() {
                let go = row.state == BookDownloadState::Done
                    && (row.keep_ready || delete_after)
                    && marks.is_read(&row.chapter)
                    && !bookmarks.contains(&row.chapter);
                if go && forget_file(&row) {
                    let _ = store.drop_book_download(row.id);
                    gone += 1;
                }
            }
        }
        if gone > 0 {
            self.publish(store, &lent.tx);
        }
        gone
    }

    /// Queue the next [`KEEP_AHEAD`] unread chapters after `after`, when keeping chapters ready
    /// is on.
    pub async fn keep_ready(&self, lent: &Lent, work: &WorkId, after: &ChapterId) {
        if !self.setup().keep_ready {
            return;
        }
        let now = lent.clock.now();
        lent.sources.adopt_plugins(&lent.plugins);
        let Ok((source, rows)) = ordered_chapters(&lent.sources, work, now).await else {
            return;
        };
        let Some(at) = rows.iter().position(|r| r.id == *after) else { return };
        let ahead = Self::picked(
            &DownloadPick::NextUnread { n: u32::try_from(KEEP_AHEAD).unwrap_or(3) },
            &rows[at + 1..],
        );
        let store = lent.sources.store();
        if self.queue(store, work, &source, &ahead, true, now) > 0 {
            self.publish(store, &lent.tx);
            self.start(lent);
        }
    }

    /// Tell the screen which of a work's chapters stand how, as the list itself says it.
    fn relist(&self, lent: &Lent, work: WorkId) {
        let lent = lent.clone();
        tokio::spawn(async move {
            let now = lent.clock.now();
            lent.sources.adopt_plugins(&lent.plugins);
            let event = match lent.sources.chapters(&work, None, now).await {
                Ok((edition, volumes)) => {
                    ReadingEvent::Chapters { work, edition, volumes, failure: None }
                }
                Err(failure) => ReadingEvent::Chapters {
                    work,
                    edition: None,
                    volumes: Vec::new(),
                    failure: Some(failure),
                },
            };
            let _ = lent.tx.send(Event::Reading(event));
        });
    }

    // -----------------------------------------------------------------------------------------
    // The queue

    /// Run the queue if it is not running and may. A queue that is held stays as it is until
    /// [`Self::configure`] and this are called again.
    pub fn start(&self, lent: &Lent) {
        if self.setup().hold {
            self.publish(lent.sources.store(), &lent.tx);
            return;
        }
        {
            let mut running = self.0.running.lock().unwrap_or_else(|e| e.into_inner());
            if *running {
                return;
            }
            *running = true;
        }
        let (this, lent) = (self.clone(), lent.clone());
        tokio::spawn(async move { this.run(lent).await });
    }

    async fn run(&self, lent: Lent) {
        let store = lent.sources.store().clone();
        let mut touched: Vec<WorkId> = Vec::new();
        loop {
            let next = {
                let mut running = self.0.running.lock().unwrap_or_else(|e| e.into_inner());
                let next = if self.setup().hold {
                    None
                } else {
                    store.next_book_download().unwrap_or_else(|e| {
                        tracing::warn!(error = %e, "the download queue was not read");
                        None
                    })
                };
                if next.is_none() {
                    *running = false;
                }
                next
            };
            let Some(row) = next else { break };
            let work = WorkId::new(row.work.clone());
            if !touched.contains(&work) {
                touched.push(work);
            }
            if !self.one(&lent, &store, row).await {
                *self.0.running.lock().unwrap_or_else(|e| e.into_inner()) = false;
                break;
            }
            tokio::time::sleep(self.0.pace).await;
        }
        self.publish(&store, &lent.tx);
        for work in touched {
            self.relist(&lent, work);
        }
    }

    /// Fetch one chapter. False when the queue should stop: the device is full.
    async fn one(&self, lent: &Lent, store: &Store, row: BookDownload) -> bool {
        let now = lent.clock.now();
        if let Err(e) = store.book_download_progress(row.id, 0, None, now) {
            tracing::warn!(error = %e, "a download was not started");
        }
        self.publish(store, &lent.tx);
        match self.fetch(lent, store, &row).await {
            Ok(done) => {
                let path = done.path.to_string_lossy().into_owned();
                if let Err(e) =
                    store.book_download_done(row.id, &path, done.pages, done.bytes, now)
                {
                    tracing::warn!(error = %e, "a finished download was not kept");
                }
                self.publish(store, &lent.tx);
                true
            }
            Err(Stop::Cancelled) => {
                let _ = std::fs::remove_file(part_of(&self.file_stem(&row)));
                self.publish(store, &lent.tx);
                true
            }
            Err(Stop::Held) => {
                let _ = std::fs::remove_file(part_of(&self.file_stem(&row)));
                let _ = store.book_download_requeue(row.id, now);
                self.publish(store, &lent.tx);
                true
            }
            Err(Stop::Failed(failure)) => {
                let _ = std::fs::remove_file(part_of(&self.file_stem(&row)));
                let full = failure == ReadingFailure::DiskFull;
                if let Err(e) = store.book_download_failed(row.id, &encode(&failure), now) {
                    tracing::warn!(error = %e, "a failed download was not kept");
                }
                self.publish(store, &lent.tx);
                !full
            }
        }
    }

    fn file_stem(&self, row: &BookDownload) -> PathBuf {
        self.0.dir.join(safe(&row.work)).join(safe(&row.chapter))
    }

    /// Whether the row is still being fetched: it can be taken out of the queue meanwhile.
    fn wanted(store: &Store, row: &BookDownload) -> bool {
        store
            .book_download(&row.work, &row.chapter)
            .ok()
            .flatten()
            .is_some_and(|now| now.id == row.id && now.state == BookDownloadState::Downloading)
    }

    async fn fetch(
        &self,
        lent: &Lent,
        store: &Store,
        row: &BookDownload,
    ) -> Result<Fetched, Stop> {
        let wanted = || Self::wanted(store, row);
        let progress = |done: u32, total: u32| {
            let _ = store.book_download_progress(row.id, done, Some(total), lent.clock.now());
            self.publish_progress(store, &lent.tx);
        };
        let job = Fetching {
            work: WorkId::new(row.work.clone()),
            chapter: ChapterId::new(row.chapter.clone()),
            source_id: row.source_id.clone(),
            stem: self.file_stem(row),
            wanted: &wanted,
            progress: &progress,
        };
        self.fetch_with(lent, &job).await
    }

    /// Fetch one chapter into a file at `job.stem`, with the sources, the free-space floor and
    /// the Wi-Fi rule a queued download has. What the fetch is for is the caller's: a queued
    /// download keeps the file as a download, a send removes it once its files are written.
    pub(crate) async fn fetch_with(
        &self,
        lent: &Lent,
        job: &Fetching<'_>,
    ) -> Result<Fetched, Stop> {
        let now = lent.clock.now();
        if self.free().is_some_and(|free| free < KEEP_FREE) {
            return Err(ReadingFailure::DiskFull.into());
        }
        lent.sources.adopt_plugins(&lent.plugins);
        let (work, chapter) = (&job.work, &job.chapter);
        let mut opened = lent
            .sources
            .open(work, chapter, Some(&job.source_id), now)
            .await
            .map_err(Stop::Failed)?;
        if !(job.wanted)() {
            return Err(Stop::Cancelled);
        }
        let stem = job.stem.clone();
        if let Some(dir) = stem.parent() {
            std::fs::create_dir_all(dir).map_err(|e| Stop::Failed(io_failure(&e, dir)))?;
        }
        let part = part_of(&stem);
        let source = job.source_id.clone();
        match std::mem::replace(&mut opened.content, ChapterContent::Text(String::new())) {
            ChapterContent::Images(pages) => {
                let total = u32::try_from(pages.len()).unwrap_or(u32::MAX);
                opened.content = ChapterContent::Images(pages);
                if total == 0 {
                    return Err(ReadingFailure::NotFound.into());
                }
                let file =
                    File::create(&part).map_err(|e| Stop::Failed(io_failure(&e, &part)))?;
                let mut cbz = kaeri_books::write::CbzWriter::new(BufWriter::new(file), None)
                    .map_err(|e| Stop::Failed(book_failure(&e, &part, &source)))?;
                for index in 0..total {
                    if self.setup().hold {
                        return Err(Stop::Held);
                    }
                    if !(job.wanted)() {
                        return Err(Stop::Cancelled);
                    }
                    if self.free().is_some_and(|free| free < KEEP_FREE) {
                        return Err(ReadingFailure::DiskFull.into());
                    }
                    let bytes = lent
                        .sources
                        .page(&mut opened, index as usize, lent.clock.now())
                        .await
                        .map_err(Stop::Failed)?;
                    cbz.add_page(&bytes)
                        .map_err(|e| Stop::Failed(book_failure(&e, &part, &source)))?;
                    (job.progress)(index + 1, total);
                }
                let mut out =
                    cbz.finish().map_err(|e| Stop::Failed(book_failure(&e, &part, &source)))?;
                out.flush().map_err(|e| Stop::Failed(io_failure(&e, &part)))?;
                drop(out);
                finish(&part, &stem.with_extension("cbz"), Some(total))
            }
            ChapterContent::Text(body) => {
                let file =
                    File::create(&part).map_err(|e| Stop::Failed(io_failure(&e, &part)))?;
                let label = job.chapter.as_str().to_owned();
                let meta = kaeri_books::write::BookMeta {
                    title: label.clone(),
                    language: "und".to_owned(),
                    ..Default::default()
                };
                let mut epub = kaeri_books::write::EpubWriter::new(
                    BufWriter::new(file),
                    meta,
                    kaeri_books::write::EpubFlavor::Epub,
                    None,
                )
                .map_err(|e| Stop::Failed(book_failure(&e, &part, &source)))?;
                epub.add_chapter(&label, &body)
                    .map_err(|e| Stop::Failed(book_failure(&e, &part, &source)))?;
                let mut out = epub
                    .finish()
                    .map_err(|e| Stop::Failed(book_failure(&e, &part, &source)))?;
                out.flush().map_err(|e| Stop::Failed(io_failure(&e, &part)))?;
                drop(out);
                finish(&part, &stem.with_extension("epub"), None)
            }
            ChapterContent::Book(book) => {
                let extension = book
                    .path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map_or_else(|| "cbz".to_owned(), str::to_ascii_lowercase);
                std::fs::copy(&book.path, &part)
                    .map_err(|e| Stop::Failed(io_failure(&e, &part)))?;
                if book.temporary {
                    let _ = std::fs::remove_file(&book.path);
                }
                finish(&part, &stem.with_extension(extension), None)
            }
        }
    }
}

/// What one fetch is for and asks while it runs.
pub(crate) struct Fetching<'a> {
    pub work: WorkId,
    pub chapter: ChapterId,
    pub source_id: String,
    /// The file's path without its extension, which the fetch picks from what it got.
    pub stem: PathBuf,
    /// Whether the fetch is still wanted: false once it was taken out of the queue or the send
    /// was cancelled.
    pub wanted: &'a (dyn Fn() -> bool + Send + Sync),
    /// Told `(pages done, pages total)` after each page.
    pub progress: &'a (dyn Fn(u32, u32) + Send + Sync),
}

pub(crate) struct Fetched {
    pub(crate) path: PathBuf,
    pub(crate) pages: Option<u32>,
    pub(crate) bytes: u64,
}

fn finish(part: &Path, to: &Path, pages: Option<u32>) -> Result<Fetched, Stop> {
    std::fs::rename(part, to).map_err(|e| Stop::Failed(io_failure(&e, to)))?;
    let bytes = std::fs::metadata(to).map(|m| m.len()).unwrap_or(0);
    Ok(Fetched { path: to.to_path_buf(), pages, bytes })
}

fn part_of(stem: &Path) -> PathBuf {
    let mut name = stem.file_name().map(std::ffi::OsStr::to_os_string).unwrap_or_default();
    name.push(".part");
    stem.with_file_name(name)
}

/// A name that is safe in any file system and tells different ids apart.
pub(crate) fn safe(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    for ch in id.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push_str(&format!("~{:x}", u32::from(ch)));
        }
    }
    out
}

/// Take a downloaded file away. True when it is gone, or was never there.
fn forget_file(row: &BookDownload) -> bool {
    let Some(path) = &row.path else { return true };
    match std::fs::remove_file(path) {
        Ok(()) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
        Err(e) => {
            tracing::warn!(error = %e, "a downloaded chapter's file was not removed");
            false
        }
    }
}

fn io_failure(e: &std::io::Error, path: &Path) -> ReadingFailure {
    // Out of room is the one the person can act on; the rest are the file's.
    if e.raw_os_error() == Some(28) || e.kind() == std::io::ErrorKind::StorageFull {
        ReadingFailure::DiskFull
    } else {
        ReadingFailure::FileMissing { path: path.to_string_lossy().into_owned() }
    }
}

fn book_failure(e: &kaeri_books::BookError, path: &Path, source: &str) -> ReadingFailure {
    match e {
        kaeri_books::BookError::Io(io) => io_failure(io, path),
        _ => ReadingFailure::Parse { source: source.to_owned() },
    }
}

// ---------------------------------------------------------------------------------------------
// Rows

fn state_of(d: &BookDownload, wifi: bool) -> ChapterDownloadState {
    match d.state {
        BookDownloadState::Queued | BookDownloadState::Removed if wifi => {
            ChapterDownloadState::WaitsForWifi
        }
        BookDownloadState::Queued | BookDownloadState::Removed => ChapterDownloadState::Queued,
        BookDownloadState::Downloading => ChapterDownloadState::Downloading {
            done: d.pages_done,
            total: d.pages_total.unwrap_or(0),
        },
        BookDownloadState::Done => ChapterDownloadState::Done,
        BookDownloadState::Failed => ChapterDownloadState::Failed {
            failure: d.error.as_deref().map_or(ReadingFailure::Offline, decode),
        },
    }
}

pub(crate) fn chapter_row(d: &BookDownload, read: bool, bookmarked: bool) -> ChapterRow {
    let id = ChapterId::new(d.chapter.clone());
    let key = id.key();
    let number = key.as_ref().and_then(|k| k.range()).map(|r| r.to_string());
    ChapterRow {
        label: number.clone().unwrap_or_else(|| d.chapter.clone()),
        number,
        volume: key.as_ref().and_then(|k| k.volume().map(str::to_owned)),
        id,
        state: if read { ChapterReadState::Read } else { ChapterReadState::Unread },
        title: None,
        published: None,
        pages: d.pages_total,
        new_chapter: false,
        download: Some(state_of(d, false)),
        versions: 1,
        bookmarked,
        version_label: d.version_label.clone(),
        read_at: None,
    }
}

/// What a failure is kept as in the row.
fn encode(failure: &ReadingFailure) -> String {
    match failure {
        ReadingFailure::Offline => "offline".into(),
        ReadingFailure::DiskFull => "disk_full".into(),
        ReadingFailure::NotFound => "not_found".into(),
        ReadingFailure::Protected => "protected".into(),
        ReadingFailure::PageFetch { status } => format!("page:{status}"),
        ReadingFailure::SourceDown { source } => format!("down:{source}"),
        ReadingFailure::Parse { source } => format!("parse:{source}"),
        ReadingFailure::SignInNeeded { what } => format!("sign_in:{what}"),
        ReadingFailure::Blocked { source, reason } => format!("blocked:{source}:{reason}"),
        ReadingFailure::FileMissing { path } => format!("file:{path}"),
        _ => "other".into(),
    }
}

fn decode(text: &str) -> ReadingFailure {
    let (kind, rest) = text.split_once(':').unwrap_or((text, ""));
    match kind {
        "offline" => ReadingFailure::Offline,
        "disk_full" => ReadingFailure::DiskFull,
        "not_found" => ReadingFailure::NotFound,
        "protected" => ReadingFailure::Protected,
        "page" => ReadingFailure::PageFetch { status: rest.parse().unwrap_or(0) },
        "parse" => ReadingFailure::Parse { source: rest.to_owned() },
        "sign_in" => ReadingFailure::SignInNeeded { what: rest.to_owned() },
        "blocked" => {
            let (source, reason) = rest.split_once(':').unwrap_or((rest, ""));
            ReadingFailure::Blocked { source: source.to_owned(), reason: reason.to_owned() }
        }
        "file" => ReadingFailure::FileMissing { path: rest.to_owned() },
        _ => ReadingFailure::SourceDown { source: rest.to_owned() },
    }
}

/// A work's chapters in reading order, and the source they come from.
pub(crate) async fn ordered_chapters(
    sources: &BookSources,
    work: &WorkId,
    now: i64,
) -> Result<(String, Vec<ChapterRow>), ReadingFailure> {
    let (edition, groups) = sources.chapters(work, None, now).await?;
    let source = edition
        .map(|e| e.source_id)
        .or_else(|| sources.edition_of(work, None).map(|(source, _)| source))
        .ok_or(ReadingFailure::NotFound)?;
    Ok((source, flatten(groups)))
}

/// The chapters of every volume, ordered by volume then number; chapters with no number last,
/// in the order the source gave them.
pub fn flatten(groups: Vec<VolumeGroup>) -> Vec<ChapterRow> {
    let mut rows: Vec<(usize, ChapterRow)> =
        groups.into_iter().flat_map(|g| g.chapters).enumerate().collect();
    let key = |row: &ChapterRow| {
        let volume =
            row.volume.as_deref().and_then(|v| v.trim().parse::<f64>().ok()).unwrap_or(0.0);
        let number = row.id.key().and_then(|k| k.range()).map(|r| r.last().milli());
        (volume, number)
    };
    rows.sort_by(|(ia, a), (ib, b)| {
        let ((va, na), (vb, nb)) = (key(a), key(b));
        va.total_cmp(&vb)
            .then_with(|| match (na, nb) {
                (Some(x), Some(y)) => x.cmp(&y),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            })
            .then(ia.cmp(ib))
    });
    rows.into_iter().map(|(_, row)| row).collect()
}

// ---------------------------------------------------------------------------------------------
// The engine's side

/// The config, the network or where the app is changed: set the queue up again, and run it if
/// it may. Says nothing to the screens unless something starts.
pub fn resume(engine: &crate::state::EngineState, services: &Services, tx: &Outbox) {
    if !engine.reading.available {
        return;
    }
    let setup = Setup::of(
        engine.phase,
        engine.network,
        engine.config.effective(),
        engine.reading.edition,
    );
    let downloads = &services.book_sources.downloads;
    let before = downloads.setup();
    if downloads.configure(setup) {
        // An engine that does only what it is sent leaves the queue as it is.
        if engine.ahead.on() {
            downloads.start(&Lent::of(services, tx));
        }
    } else if before != setup {
        // Rows that wait for Wi-Fi say so; with none, there is nothing to tell.
        let store = services.book_sources.store();
        if store.book_downloads(None).is_ok_and(|rows| !rows.is_empty()) {
            downloads.publish(store, tx);
        }
    }
}

/// The engine has started: fetches cut short are queued again, and the queue runs.
pub fn launched(engine: &crate::state::EngineState, services: &Services, tx: &Outbox) {
    if !engine.reading.available {
        return;
    }
    services
        .book_sources
        .downloads
        .recover(services.book_sources.store(), services.clock.now());
    resume(engine, services, tx);
}

#[cfg(test)]
mod tests;
