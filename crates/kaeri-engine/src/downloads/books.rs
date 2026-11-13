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
//! always holds a whole file. Each page of a chapter of images is kept as it lands
//! (see `kept`), so a fetch cut short fetches only the pages it lacks when it runs again.
//!
//! **Wi-Fi only until told otherwise.** On cellular, in Low Data Mode, offline, or with the app
//! off screen (but for a downloads window) the queue holds; rows show as waiting for Wi-Fi
//! where that is why, and go on by themselves when the network or the setting changes. Until
//! the front end first says the network, the queue holds too: at launch it may be cellular.
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
    config::{Config, NewChapters},
};
use kaeri_model::{
    Msg, ReadingMsg,
    command::AppState,
    event::Event,
    medium::AppEdition,
    reading::{
        BookDownloadRow, BookDownloadsView, ChapterDownloadState, ChapterReadState, ChapterRow,
        DownloadPick, ReadingEvent, ReadingFailure, StorageMeter, VolumeGroup, WriteProblem,
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
const PROGRESS_EVERY: Duration =
    if cfg!(test) { Duration::ZERO } else { Duration::from_secs(2) };
/// How often a queue a full device stopped looks for room again.
const ROOM_CHECK: Duration =
    if cfg!(test) { Duration::from_millis(5) } else { Duration::from_secs(10) };
/// How much more than when the queue paused has to be free before it goes on by itself, so a
/// write the device refused with room still free is not tried again for every byte freed.
const ROOM_MORE: u64 = 64 * 1_000_000;
/// While the queue waits for room, a change in free space this big is told, so the Downloads
/// meter follows what the person frees.
const FREE_STEP: u64 = 10 * 1_000_000;
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
    /// What a chapter list is finished with, as the series page has it.
    pub meta: kaeri_meta::Meta,
    pub offline: Arc<std::sync::atomic::AtomicBool>,
}

impl Lent {
    pub fn of(services: &Services, tx: &Outbox) -> Self {
        Self {
            sources: services.book_sources.clone(),
            plugins: services.book_plugins.clone(),
            clock: services.clock.clone(),
            tx: tx.clone(),
            meta: services.meta.clone(),
            offline: services.offline.clone(),
        }
    }

    pub(crate) fn of_kit(kit: &crate::read_catalog::Kit, tx: &Outbox) -> Self {
        Self {
            sources: kit.sources.clone(),
            plugins: kit.plugins.clone(),
            clock: kit.clock.clone(),
            tx: tx.clone(),
            meta: kit.meta.clone(),
            offline: kit.offline.clone(),
        }
    }

    fn kit(&self) -> crate::read_catalog::Kit {
        crate::read_catalog::Kit {
            meta: self.meta.clone(),
            store: self.sources.store().clone(),
            sources: self.sources.clone(),
            plugins: self.plugins.clone(),
            clock: self.clock.clone(),
            offline: self.offline.clone(),
        }
    }

    /// A work's chapters as the series page lists them, each row with its words.
    fn chapters_event(
        &self,
        work: WorkId,
        listed: (Option<kaeri_model::reading::BookEditionRow>, Vec<VolumeGroup>),
    ) -> ReadingEvent {
        let (edition, mut volumes) = listed;
        let now = self.clock.now();
        crate::read_catalog::series::finish(
            &self.kit(),
            &work,
            edition.as_ref(),
            &mut volumes,
            now,
        );
        ReadingEvent::Chapters { work, edition, volumes, failure: None }
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
    /// Which series' new chapters the update check queues.
    pub new_chapters: NewChapters,
    pub delete_after: bool,
    /// On mobile data, and chapters download on it.
    pub mobile: bool,
    /// Chapters download only on Wi-Fi, as the settings say.
    pub wifi_only: bool,
    /// Say when a run of downloads is done: the Downloads finished notification.
    pub tell_done: bool,
}

impl Setup {
    /// Where the queue starts, before the engine has set it up: held, as the network is not
    /// said yet.
    pub const UNSAID: Self = Self {
        hold: true,
        wifi: false,
        offline: false,
        keep_ready: false,
        new_chapters: NewChapters::Off,
        delete_after: false,
        mobile: false,
        wifi_only: false,
        tell_done: false,
    };

    /// The setup for the `network` the front end last said, `None` before it has said one.
    pub fn of(
        phase: AppState,
        network: Option<Network>,
        config: &Config,
        edition: AppEdition,
    ) -> Self {
        let reading = &config.reading;
        let waits = crate::phases::waits_for_wifi(network, reading.downloads_on_cellular);
        // Before the network is said the rows only wait: nothing says it is for Wi-Fi yet.
        let wifi = waits && network.is_some();
        let offline = network.is_some_and(|n| n.offline);
        let away = matches!(phase, AppState::Background { .. });
        let new_chapters = reading.new_chapters(new_chapters_default(edition));
        Self {
            hold: waits || offline || away,
            wifi,
            offline,
            keep_ready: new_chapters != NewChapters::Off,
            new_chapters,
            delete_after: reading.delete_after_reading,
            mobile: network.is_some_and(|n| n.cellular && !n.offline) && !wifi,
            wifi_only: !reading.downloads_on_cellular,
            tell_done: config.notifications.downloads,
        }
    }
}

/// Which series' new chapters download where the person has not said: the Reading list's in
/// the e-reader edition, none elsewhere.
pub fn new_chapters_default(edition: AppEdition) -> NewChapters {
    if matches!(edition, AppEdition::ReadOnly) {
        NewChapters::Reading
    } else {
        NewChapters::Off
    }
}

/// The reading download queue. Cheap to clone.
#[derive(Clone)]
pub struct BookDownloads(Arc<Inner>);

struct Inner {
    dir: PathBuf,
    pace: Duration,
    running: Mutex<bool>,
    /// The queue paused for a full device: every chapter it holds, the one that met the full
    /// device too, stays queued and waits for room, and the queue goes on by itself once there
    /// is some.
    full: Mutex<bool>,
    /// What was free when the queue stopped for a full device, when that could be told.
    full_free: Mutex<Option<u64>>,
    /// Something looks for room every [`ROOM_CHECK`] while the queue is stopped for it.
    watching: Mutex<bool>,
    setup: Mutex<Setup>,
    /// Told each time the setup changes, for a send that waits on it.
    changed: tokio::sync::watch::Sender<Setup>,
    /// What a test takes to be free on the device, in place of asking the disk.
    free: Mutex<Option<u64>>,
    told: Mutex<Option<Instant>>,
    /// The title language setting, for the series' names; English first until it is given.
    titles: std::sync::OnceLock<kaeri_meta::titles::Titles>,
    /// The pages of the chapters being fetched, for a reader that opens one meanwhile.
    shares: shared::Shares,
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
            full: Mutex::new(false),
            full_free: Mutex::new(None),
            watching: Mutex::new(false),
            setup: Mutex::new(Setup::UNSAID),
            changed: tokio::sync::watch::channel(Setup::UNSAID).0,
            free: Mutex::new(None),
            told: Mutex::new(None),
            titles: std::sync::OnceLock::new(),
            shares: shared::Shares::default(),
        }))
    }

    /// Name series in the title language `titles` follows.
    pub fn use_titles(&self, titles: kaeri_meta::titles::Titles) {
        let _ = self.0.titles.set(titles);
    }

    /// The form of a title to show now.
    fn title_form(&self) -> kaeri_store::shelf::TitleForm {
        self.0.titles.get().map(crate::data::title_form).unwrap_or_default()
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
    /// The pages a fetch kept stay for it to pick up, while its chapter is still to be fetched.
    pub fn recover(&self, store: &Store, now: i64) {
        match store.book_downloads_recover(now) {
            Ok(0) => {}
            Ok(n) => tracing::info!(n, "chapter downloads cut short are queued again"),
            Err(e) => tracing::warn!(error = %e, "downloads cut short were not queued again"),
        }
        let Ok(rows) = store.book_downloads(None) else { return };
        self.move_legacy_pages(&rows);
        let to_fetch: HashSet<PathBuf> = rows
            .iter()
            .filter(|r| {
                matches!(r.state, BookDownloadState::Queued | BookDownloadState::Failed)
            })
            .map(|r| pages_of(&self.file_stem(r)))
            .collect();
        let Ok(works) = std::fs::read_dir(&self.0.dir) else { return };
        for work in works.flatten() {
            let Ok(files) = std::fs::read_dir(work.path()) else { continue };
            for file in files.flatten() {
                let path = file.path();
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default();
                if ext == "part" {
                    let _ = std::fs::remove_file(&path);
                } else if ext == "pages" && !to_fetch.contains(&path) {
                    KeptPages::remove(&path);
                }
            }
        }
    }

    /// Pages an earlier version kept under its old names move to the names this one uses, so a
    /// fetch it cut short goes on from them. Its finished files stay where they are: each row
    /// keeps its file's path. Only an old name no other row shares, where case is ignored,
    /// moves; pages under a shared one stay and are fetched again. Each move is one rename, so
    /// a launch cut short leaves the pages either moved or where they were, for the next one.
    fn move_legacy_pages(&self, rows: &[BookDownload]) {
        let legacy = |r: &BookDownload| {
            let name = kaeri_core::file_name::legacy;
            self.0.dir.join(name(&r.work)).join(name(&r.chapter))
        };
        let key = |path: &Path| path.to_string_lossy().to_lowercase();
        let mut owners: HashMap<String, usize> = HashMap::new();
        for row in rows {
            *owners.entry(key(&legacy(row))).or_default() += 1;
        }
        let current: HashSet<String> = rows.iter().map(|r| key(&self.file_stem(r))).collect();
        for row in rows {
            if !matches!(row.state, BookDownloadState::Queued | BookDownloadState::Failed) {
                continue;
            }
            let (old, new) = (legacy(row), self.file_stem(row));
            let old_key = key(&old);
            if old == new || owners.get(&old_key) != Some(&1) || current.contains(&old_key) {
                continue;
            }
            let (from, to) = (pages_of(&old), pages_of(&new));
            if !from.is_dir() || to.exists() {
                continue;
            }
            let moved = to
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::rename(&from, &to));
            if let Err(e) = moved {
                tracing::warn!(error = %e, "pages kept under an old name were not moved");
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
        let room = self.waits_room();
        let rows = store.book_downloads(None).unwrap_or_default();
        let mut marks: HashMap<String, kaeri_store::reading::ReadMarks> = HashMap::new();
        let mut shown = Vec::with_capacity(rows.len());
        let form = self.title_form();
        for d in &rows {
            let read = marks
                .entry(d.work.clone())
                .or_insert_with(|| crate::read_catalog::chapters::read_marks(store, &d.work))
                .is_read(&d.chapter);
            let bookmarked = store.bookmarks(&d.work).unwrap_or_default().contains(&d.chapter);
            let kind = store.book_work_kind(&d.work).ok().flatten();
            let medium = kind
                .as_ref()
                .and_then(|kind| kaeri_core::media::Medium::parse(&kind.medium))
                .unwrap_or(kaeri_core::media::Medium::Manga);
            let title =
                kind.and_then(|kind| kind.title_in(form)).unwrap_or_else(|| d.work.clone());
            let state = state_of(d, wifi);
            let mut chapter = chapter_row(d, read, bookmarked);
            // A download row carries no dates, so the clock plays no part in its words.
            let shown_as = ChapterRow { download: Some(state.clone()), ..chapter.clone() };
            chapter.line = crate::read_catalog::rows::line(
                &shown_as,
                crate::read_catalog::rows::Kept::Source,
                medium,
                0,
                &jiff::tz::TimeZone::UTC,
            );
            if room {
                crate::read_catalog::rows::waits_room(&mut chapter.line);
            }
            shown.push(BookDownloadRow {
                work: WorkId::new(d.work.clone()),
                title,
                chapter,
                state,
            });
        }
        let (used, chapters) = store.book_download_totals().unwrap_or((0, 0));
        let free = self.free().unwrap_or(0);
        let (series, read_bytes, kept) = sizes::series(store, &rows, (wifi, room), form);
        BookDownloadsView {
            rows: shown,
            meter: StorageMeter { used, free, chapters, read_bytes },
            series,
            total: sizes::total(kept, used),
            stopped_full: room,
        }
    }

    /// Tell the screens how downloads stand.
    pub fn publish(&self, store: &Store, tx: &Outbox) {
        let _ = tx.send(Event::Reading(ReadingEvent::Downloads(self.view(store))));
    }

    /// Tell how downloads stand unless that was told a moment ago. Says whether it told.
    fn publish_progress(&self, store: &Store, tx: &Outbox) -> bool {
        let mut told = self.0.told.lock().unwrap_or_else(|e| e.into_inner());
        if told.is_some_and(|at| at.elapsed() < PROGRESS_EVERY) {
            return false;
        }
        *told = Some(Instant::now());
        drop(told);
        self.publish(store, tx);
        true
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

    /// The chapters asking for `pick` fetches. Next unread counts from the reader's place, the
    /// chapter in progress or the one after the last read, and passes over chapters already on
    /// this device or on their way: a second Next 5 unread fetches the five after the first.
    /// Unread chapters behind the place, the person passed over, are left out. Every other pick
    /// is [`Self::picked`].
    pub fn to_fetch(pick: &DownloadPick, rows: &[ChapterRow]) -> Vec<Wanted> {
        let DownloadPick::NextUnread { n } = pick else { return Self::picked(pick, rows) };
        let fetched = |r: &ChapterRow| {
            r.download
                .as_ref()
                .is_some_and(|d| !matches!(d, ChapterDownloadState::Failed { .. }))
        };
        rows[place(rows)..]
            .iter()
            .filter(|r| !matches!(r.state, ChapterReadState::Read) && !fetched(r))
            .take(*n as usize)
            .map(|r| Wanted { chapter: r.id.clone(), version_label: r.version_label.clone() })
            .collect()
    }

    /// Ask for the chapters `pick` names. Lists the work's chapters first, so this answers
    /// after the source does; a source that cannot be asked ends in `Failed` for the screen
    /// that asked.
    pub async fn download(&self, lent: &Lent, work: WorkId, pick: DownloadPick) {
        let now = lent.clock.now();
        lent.sources.plugins_in(&lent.plugins, None).await;
        let listed = ordered_chapters(&lent.sources, &work, now).await;
        let (source, rows) = match listed {
            Ok(found) => found,
            Err(failure) => {
                let about = kaeri_model::reading::ReadingAbout::Downloads;
                let _ = lent.tx.send(Event::Reading(ReadingEvent::Failed { about, failure }));
                return;
            }
        };
        let wanted = Self::to_fetch(&pick, &rows);
        let store = lent.sources.store();
        let listers = Listers::of(&lent.sources, &work, &source);
        let (added, unlisted) =
            self.queue_listed(store, &work, (&listers, &rows), &wanted, false, now);
        // A chapter the series page listed from a source that no longer has it.
        if unlisted > 0 {
            let about = kaeri_model::reading::ReadingAbout::Downloads;
            let failure = ReadingFailure::NotFound;
            let _ = lent.tx.send(Event::Reading(ReadingEvent::Failed { about, failure }));
        }
        if added > 0 || wanted.len() > unlisted {
            self.publish(store, &lent.tx);
            self.start(lent);
        }
    }

    /// Queue `wanted`, in their order, each from the source `listers` gives its row in `rows`.
    /// Says how many were newly queued, and how many no source lists any more.
    fn queue_listed(
        &self,
        store: &Store,
        work: &WorkId,
        (listers, rows): (&Listers, &[ChapterRow]),
        wanted: &[Wanted],
        keep_ready: bool,
        now: i64,
    ) -> (usize, usize) {
        let (mut added, mut unlisted) = (0, 0);
        for want in wanted {
            let row = rows.iter().find(|r| r.id == want.chapter);
            match row.map_or(Some(listers.edition.as_str()), |row| listers.of_row(row)) {
                Some(source) => {
                    added += self.queue(
                        store,
                        work,
                        source,
                        std::slice::from_ref(want),
                        keep_ready,
                        now,
                    )
                }
                None => unlisted += 1,
            }
        }
        (added, unlisted)
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

    /// Delete read: the downloads of a work's read chapters, bookmarked ones spared, whatever
    /// the settings say.
    pub fn delete_read(&self, lent: &Lent, work: &WorkId) -> usize {
        let store = lent.sources.store();
        let marks = crate::read_catalog::chapters::read_marks(store, work.as_str());
        let bookmarks: HashSet<String> =
            store.bookmarks(work.as_str()).unwrap_or_default().into_iter().collect();
        let read: Vec<String> = store
            .book_downloads(Some(work.as_str()))
            .unwrap_or_default()
            .into_iter()
            .filter(|d| d.state == BookDownloadState::Done)
            .filter(|d| marks.is_read(&d.chapter) && !bookmarks.contains(&d.chapter))
            .map(|d| d.chapter)
            .collect();
        if read.is_empty() {
            self.publish(store, &lent.tx);
            return 0;
        }
        let removed = self.remove(store, work.as_str(), &read, true, lent.clock.now());
        self.publish(store, &lent.tx);
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
                    keep_parts(store, &row.work, &row.chapter, None);
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

    /// The reader fetches `chapter` itself, as the queue had not started it: a chapter waiting
    /// only to be kept ready leaves the queue in the same step, so the source is not asked for
    /// it again. One the person asked for stays. False when the queue started it meanwhile: the
    /// reader joins that fetch instead.
    pub(crate) fn reader_fetches(
        &self,
        store: &Store,
        tx: &Outbox,
        (work, chapter): (&WorkId, &ChapterId),
        now: i64,
    ) -> bool {
        if store.unqueue_kept_ready(work.as_str(), chapter.as_str(), now).unwrap_or(false) {
            self.publish(store, tx);
            return true;
        }
        let row = store.book_download(work.as_str(), chapter.as_str()).ok().flatten();
        !row.is_some_and(|r| r.state == BookDownloadState::Downloading)
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
            let marks = crate::read_catalog::chapters::read_marks(store, &work);
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
        lent.sources.plugins_in(&lent.plugins, None).await;
        lent.sources.adopt_plugins(&lent.plugins);
        // The list the reader opened from serves while it is fresh: a chapter read is no reason
        // to ask the source for the whole list again.
        let listed = match lent.sources.kept_listing(work, now, crate::read_sources::LIST_FRESH)
        {
            Some((edition, groups)) => edition
                .map(|e| e.source_id)
                .or_else(|| lent.sources.edition_of(work, None).map(|(source, _)| source))
                .map(|source| (source, flatten(groups)))
                .ok_or(ReadingFailure::NotFound),
            None => ordered_chapters(&lent.sources, work, now).await,
        };
        let Ok((source, rows)) = listed else { return };
        let Some(at) = rows.iter().position(|r| r.id == *after) else { return };
        let ahead = Self::picked(
            &DownloadPick::NextUnread { n: u32::try_from(KEEP_AHEAD).unwrap_or(3) },
            &rows[at + 1..],
        );
        let store = lent.sources.store();
        let listers = Listers::of(&lent.sources, work, &source);
        if self.queue_listed(store, work, (&listers, &rows), &ahead, true, now).0 > 0 {
            self.publish(store, &lent.tx);
            self.start(lent);
        }
    }

    /// Tell the series page how a work's chapters stand while its downloads run, from the list
    /// as kept: a run never asks the source again only to say how far it got. Sent from the run
    /// itself, so one told later never lands before one told earlier.
    fn relist_kept(&self, lent: &Lent, work: &str) {
        let work = WorkId::new(work);
        let Some(listed) = lent.sources.kept_listing(&work, lent.clock.now(), i64::MAX) else {
            return;
        };
        let _ = lent.tx.send(Event::Reading(lent.chapters_event(work, listed)));
    }

    /// Tell the screen which of a work's chapters stand how, as the list itself says it.
    fn relist(&self, lent: &Lent, work: WorkId) {
        let lent = lent.clone();
        tokio::spawn(async move {
            let now = lent.clock.now();
            lent.sources.plugins_in(&lent.plugins, None).await;
            lent.sources.adopt_plugins(&lent.plugins);
            // Only the downloads changed: the list as kept says the rest.
            let listed = match lent.sources.kept_listing(&work, now, i64::MAX) {
                Some(kept) => Ok(kept),
                None => lent.sources.chapters(&work, None, now).await,
            };
            let event = match listed {
                Ok(listed) => lent.chapters_event(work, listed),
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
    /// [`Self::configure`] and this are called again. With chapters queued and the device short
    /// of room, as after a relaunch with the disk still full, the queue pauses for room without
    /// asking a source, and goes on by itself once there is some.
    pub fn start(&self, lent: &Lent) {
        let store = lent.sources.store();
        if self.setup().hold {
            self.publish(store, &lent.tx);
            return;
        }
        {
            let mut running = self.0.running.lock().unwrap_or_else(|e| e.into_inner());
            if *running {
                return;
            }
            let queued = || store.next_book_download().ok().flatten().is_some();
            if self.short_of_room() && queued() {
                drop(running);
                self.pause_full();
                self.publish(store, &lent.tx);
                self.watch_room(lent);
                return;
            }
            *running = true;
        }
        *self.0.full.lock().unwrap_or_else(|e| e.into_inner()) = false;
        let (this, lent) = (self.clone(), lent.clone());
        tokio::spawn(async move { this.run(lent).await });
    }

    /// The queue pauses for room: what is free now is kept, to tell when room came back.
    fn pause_full(&self) {
        *self.0.full_free.lock().unwrap_or_else(|e| e.into_inner()) = self.free();
        *self.0.full.lock().unwrap_or_else(|e| e.into_inner()) = true;
    }

    /// The queue paused for a full device and has not started again.
    pub(crate) fn waits_room(&self) -> bool {
        *self.0.full.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn short_of_room(&self) -> bool {
        self.free().is_some_and(|free| free < KEEP_FREE)
    }

    /// Room came back since the queue paused for a full device: what is free now is known, is
    /// at least [`KEEP_FREE`], and is [`ROOM_MORE`] more than when it paused.
    fn room_back(&self) -> bool {
        let then = *self.0.full_free.lock().unwrap_or_else(|e| e.into_inner());
        self.free().is_some_and(|now| {
            now >= KEEP_FREE && then.is_none_or(|then| now >= then.saturating_add(ROOM_MORE))
        })
    }

    /// Look for room every [`ROOM_CHECK`] while the queue is paused for it, and start it once
    /// there is some. Meanwhile a change in free space is told, for the Downloads meter. One
    /// looks at a time; it ends when the queue starts by other means.
    fn watch_room(&self, lent: &Lent) {
        {
            let mut watching = self.0.watching.lock().unwrap_or_else(|e| e.into_inner());
            if *watching {
                return;
            }
            *watching = true;
        }
        let (this, lent) = (self.clone(), lent.clone());
        tokio::spawn(async move {
            let mut told = this.free();
            loop {
                tokio::time::sleep(ROOM_CHECK).await;
                let free = this.free();
                if free.unwrap_or(0).abs_diff(told.unwrap_or(0)) >= FREE_STEP {
                    told = free;
                    this.publish(lent.sources.store(), &lent.tx);
                }
                // Decided under the lock a stop takes to ask for a look, so a stop just after
                // this one ends gets its own.
                let roomy = {
                    let mut watching =
                        this.0.watching.lock().unwrap_or_else(|e| e.into_inner());
                    let roomy = this.room_back();
                    if !this.waits_room() || roomy {
                        *watching = false;
                    }
                    if !this.waits_room() {
                        return;
                    }
                    roomy
                };
                if roomy {
                    this.start(&lent);
                    return;
                }
            }
        });
    }

    async fn run(&self, lent: Lent) {
        let store = lent.sources.store().clone();
        let mut touched: Vec<WorkId> = Vec::new();
        // The chapters the person asked for that finished in this run, by series.
        let mut finished: Vec<(String, usize)> = Vec::new();
        let mut stopped = false;
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
            let asked = !row.keep_ready;
            let work = row.work.clone();
            let (go_on, done) = self.one(&lent, &store, row).await;
            if done && asked {
                match finished.iter_mut().find(|(w, _)| *w == work) {
                    Some((_, n)) => *n += 1,
                    None => finished.push((work, 1)),
                }
            }
            if !go_on {
                // Full first: a start that finds the queue no longer running clears it.
                self.pause_full();
                *self.0.running.lock().unwrap_or_else(|e| e.into_inner()) = false;
                self.watch_room(&lent);
                stopped = true;
                break;
            }
            tokio::time::sleep(self.0.pace).await;
        }
        self.publish(&store, &lent.tx);
        for work in touched {
            self.relist(&lent, work);
        }
        // Only a run that emptied the queue is done: a held or full one has more to come.
        let held = self.setup().hold;
        if self.setup().tell_done && !stopped && !held && !finished.is_empty() {
            let _ =
                lent.tx.send(Event::Reading(done_notice(&store, &finished, self.title_form())));
        }
    }

    /// Fetch one chapter. Says whether the queue goes on, which it does not when the device is
    /// full, and whether the chapter is now downloaded.
    async fn one(&self, lent: &Lent, store: &Store, row: BookDownload) -> (bool, bool) {
        let now = lent.clock.now();
        if let Err(e) = store.book_download_progress(row.id, 0, None, now) {
            tracing::warn!(error = %e, "a download was not started");
        }
        self.publish(store, &lent.tx);
        self.relist_kept(lent, &row.work);
        let outcome = self.ended(lent, store, &row, now).await;
        // The series page hears each chapter land, not the whole run at its end.
        self.relist_kept(lent, &row.work);
        outcome
    }

    /// Fetch the chapter and keep how that ended, as [`Self::one`] says it.
    async fn ended(
        &self,
        lent: &Lent,
        store: &Store,
        row: &BookDownload,
        now: i64,
    ) -> (bool, bool) {
        match self.fetch(lent, store, row).await {
            Ok(done) => {
                let path = done.path.to_string_lossy().into_owned();
                let kept =
                    store.book_download_fetched(row.id, &path, done.pages, done.bytes, now);
                let kept = kept.unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "a finished download was not kept");
                    true
                });
                if !kept {
                    // Cancelled or deleted while its last page or its file was made: the file
                    // goes with the row, and a row kept as removed stays removed.
                    let _ = std::fs::remove_file(&done.path);
                }
                self.publish(store, &lent.tx);
                (true, kept)
            }
            Err(Stop::Cancelled) => {
                let _ = std::fs::remove_file(part_of(&self.file_stem(row)));
                self.publish(store, &lent.tx);
                (true, false)
            }
            Err(Stop::Held) => {
                let _ = std::fs::remove_file(part_of(&self.file_stem(row)));
                let _ = store.book_download_requeue(row.id, now);
                self.publish(store, &lent.tx);
                (true, false)
            }
            // A full device pauses the queue: the chapter goes back in it, its pages kept for
            // when there is room, and waits with the rest.
            Err(Stop::Failed(ReadingFailure::DiskFull)) => {
                let _ = std::fs::remove_file(part_of(&self.file_stem(row)));
                if let Err(e) = store.book_download_requeue(row.id, now) {
                    tracing::warn!(error = %e, "a download paused for room was not queued again");
                }
                (false, false)
            }
            Err(Stop::Failed(failure)) => {
                let _ = std::fs::remove_file(part_of(&self.file_stem(row)));
                if let Err(e) = store.book_download_failed(row.id, &encode(&failure), now) {
                    tracing::warn!(error = %e, "a failed download was not kept");
                }
                self.publish(store, &lent.tx);
                (true, false)
            }
        }
    }

    /// The first page of a chapter being fetched, once it has landed and been kept: for the
    /// reader's end page, which shows it before the chapter is done.
    pub(crate) fn first_kept_page(&self, work: &str, chapter: &str) -> Option<PathBuf> {
        let dir = pages_of(&self.0.dir.join(safe(work)).join(safe(chapter)));
        std::fs::read_dir(dir).ok()?.flatten().map(|entry| entry.path()).find(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("00000-") && n.ends_with(".page"))
        })
    }

    /// The pages of a chapter of pictures being fetched now, for the reader to share: it takes
    /// the pages kept and those still to come from them, and fetches any other only once.
    pub(crate) fn sharing(&self, work: &str, chapter: &str) -> Option<Arc<SharedPages>> {
        self.0.shares.of(&pages_of(&self.0.dir.join(safe(work)).join(safe(chapter))))
    }

    /// Whether the queue is about to fetch `chapter`: it runs, fetches nothing now, and takes
    /// this chapter next. One waiting behind another download, or in a queue that does not run,
    /// is not.
    pub(crate) fn starts_soon(&self, store: &Store, work: &str, chapter: &str) -> bool {
        if self.setup().hold || !*self.0.running.lock().unwrap_or_else(|e| e.into_inner()) {
            return false;
        }
        let rows = store.book_downloads(None).unwrap_or_default();
        if rows.iter().any(|r| r.state == BookDownloadState::Downloading) {
            return false;
        }
        store
            .next_book_download()
            .ok()
            .flatten()
            .is_some_and(|next| next.work == work && next.chapter == chapter)
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
            // The series page counts the pages with Downloads, as often as Downloads does.
            if self.publish_progress(store, &lent.tx) {
                self.relist_kept(lent, &row.work);
            }
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
        lent.sources.plugins_in(&lent.plugins, Some(&job.source_id)).await;
        let (work, chapter) = (&job.work, &job.chapter);
        // Taken out of the queue as it started, by a reader fetching it itself: the source is
        // not asked for it.
        if !(job.wanted)() {
            return Err(Stop::Cancelled);
        }
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
        // The chapter's language goes into its file, so reading it offline and sending it on
        // set it as that language, as reading it from the source does.
        let language = opened
            .chapter
            .language
            .as_deref()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_owned);
        match std::mem::replace(&mut opened.content, ChapterContent::Text(String::new())) {
            ChapterContent::Images(pages) => {
                let total = u32::try_from(pages.len()).unwrap_or(u32::MAX);
                opened.content = ChapterContent::Images(pages);
                if total == 0 {
                    return Err(ReadingFailure::NotFound.into());
                }
                // Each page is kept as it lands, so a fetch cut short fetches only the pages it
                // lacks next time.
                let dir = pages_of(&stem);
                // A reader opening the chapter meanwhile shares the pages: each is fetched once,
                // by whichever asks first. One still holding them from before is shared again.
                let kept = self
                    .0
                    .shares
                    .open(&dir, total, opened.clone())
                    .map_err(|e| Stop::Failed(io_failure(&e, &dir)))?;
                for index in 0..total {
                    if self.setup().hold {
                        return Err(Stop::Held);
                    }
                    if !(job.wanted)() {
                        kept.let_go();
                        return Err(Stop::Cancelled);
                    }
                    if !kept.has(index) {
                        if self.free().is_some_and(|free| free < KEEP_FREE) {
                            return Err(ReadingFailure::DiskFull.into());
                        }
                        let now = lent.clock.now();
                        kept.page(&lent.sources, index, now, false)
                            .await
                            .map_err(Stop::Failed)?;
                        if !kept.has(index) {
                            let failure = kept.unkept(index).unwrap_or_else(|| {
                                let reason = format!("page {index} was not kept");
                                ReadingFailure::Other { reason }
                            });
                            return Err(Stop::Failed(failure));
                        }
                    }
                    (job.progress)(index + 1, total);
                }
                let file =
                    File::create(&part).map_err(|e| Stop::Failed(io_failure(&e, &part)))?;
                let info = language.clone().map(|language| kaeri_books::ComicInfo {
                    language: Some(language),
                    ..kaeri_books::ComicInfo::default()
                });
                let mut cbz =
                    kaeri_books::write::CbzWriter::new(BufWriter::new(file), info.as_ref())
                        .map_err(|e| Stop::Failed(book_failure(&e, &part, &source)))?;
                // The edge pages are fingerprinted on the way, for the series' credit pages.
                let mut prints = crate::reading::credits::Recorder::new(&opened.spans());
                for index in 0..total {
                    let bytes = kept.read(index).map_err(|_| {
                        let path = dir.to_string_lossy().into_owned();
                        Stop::Failed(ReadingFailure::FileMissing { path })
                    })?;
                    prints.page(index, &bytes).await;
                    cbz.add_page(&bytes)
                        .map_err(|e| Stop::Failed(book_failure(&e, &part, &source)))?;
                }
                prints.keep(
                    lent.sources.store(),
                    work.as_str(),
                    chapter.as_str(),
                    opened.group.as_deref(),
                    lent.clock.now(),
                );
                let parts = crate::reading::feed::SourceParts::of(&opened);
                keep_parts(
                    lent.sources.store(),
                    work.as_str(),
                    chapter.as_str(),
                    parts.as_ref(),
                );
                let mut out =
                    cbz.finish().map_err(|e| Stop::Failed(book_failure(&e, &part, &source)))?;
                out.flush().map_err(|e| Stop::Failed(io_failure(&e, &part)))?;
                drop(out);
                let done = finish(&part, &stem.with_extension("cbz"), Some(total));
                if done.is_ok() {
                    kept.let_go();
                }
                done
            }
            ChapterContent::Text(body) => {
                // Its title said once, as the reader shows it from the source.
                let mut body = crate::reading::novel::heading_once(&body, &opened.chapter);
                // Its pictures go inside the book, so it reads with no network.
                let pictures = lent.sources.pictures_in(&source, &mut body).await;
                let file =
                    File::create(&part).map_err(|e| Stop::Failed(io_failure(&e, &part)))?;
                let label = job.chapter.as_str().to_owned();
                let meta = kaeri_books::write::BookMeta {
                    title: label.clone(),
                    language: language.unwrap_or_else(|| "und".to_owned()),
                    ..Default::default()
                };
                let mut epub = kaeri_books::write::EpubWriter::new(
                    BufWriter::new(file),
                    meta,
                    kaeri_books::write::EpubFlavor::Epub,
                    None,
                )
                .map_err(|e| Stop::Failed(book_failure(&e, &part, &source)))?;
                for picture in &pictures {
                    epub.add_resource(&picture.path, picture.media_type, &picture.bytes)
                        .map_err(|e| Stop::Failed(book_failure(&e, &part, &source)))?;
                }
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

/// The meta key a downloaded chapter's parts are kept under.
fn parts_key(work: &str, chapter: &str) -> String {
    format!("book_download_parts:{work}/{chapter}")
}

/// Keep the parts a chapter was downloaded in, or forget them for one listed whole.
fn keep_parts(
    store: &Store,
    work: &str,
    chapter: &str,
    parts: Option<&crate::reading::feed::SourceParts>,
) {
    let key = parts_key(work, chapter);
    let kept = match parts.and_then(|p| serde_json::to_string(p).ok()) {
        Some(text) => store.set_meta_text(&key, &text),
        None => store.delete_meta(&key),
    };
    if let Err(e) = kept {
        tracing::warn!(error = %e, "a downloaded chapter's parts were not kept");
    }
}

/// The parts a chapter was downloaded in; `None` for one downloaded whole.
pub(crate) fn kept_parts(
    store: &Store,
    work: &str,
    chapter: &str,
) -> Option<crate::reading::feed::SourceParts> {
    let text = store.get_meta_text(&parts_key(work, chapter)).ok()??;
    serde_json::from_str(&text).ok()
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

/// The file or folder name for a work's id or a chapter's key: short enough for any file
/// system, and never another id's ([`kaeri_core::file_name::of_id`]).
pub(crate) fn safe(id: &str) -> String {
    kaeri_core::file_name::of_id(id)
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

/// Why writing `path` failed, in the reader's terms: out of room, a folder that is gone, a name
/// the storage does not take, storage that may not be written, or another error.
pub(crate) fn io_failure(e: &std::io::Error, path: &Path) -> ReadingFailure {
    use std::io::ErrorKind as K;
    let path = path.to_string_lossy().into_owned();
    let problem = match e.kind() {
        _ if e.raw_os_error() == Some(28) => return ReadingFailure::DiskFull,
        K::StorageFull | K::QuotaExceeded => return ReadingFailure::DiskFull,
        K::NotFound => return ReadingFailure::FileMissing { path },
        K::InvalidFilename => WriteProblem::Name,
        K::PermissionDenied | K::ReadOnlyFilesystem => WriteProblem::Denied,
        _ => WriteProblem::Other,
    };
    ReadingFailure::CannotWrite { path, problem }
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
        line: Vec::new(),
        gone_from_source: false,
        parts: 1,
        other_group: false,
        missing_parts: Vec::new(),
        part_groups: Vec::new(),
        group_language: None,
        other_source: None,
        on_device: false,
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
        ReadingFailure::SourceOff { source } => format!("off:{source}"),
        ReadingFailure::Parse { source } => format!("parse:{source}"),
        ReadingFailure::SignInNeeded { what } => format!("sign_in:{what}"),
        ReadingFailure::Blocked { source, reason } => format!("blocked:{source}:{reason}"),
        ReadingFailure::FileMissing { path } => format!("file:{path}"),
        ReadingFailure::CannotWrite { path, problem } => {
            let problem = match problem {
                WriteProblem::Name => "name",
                WriteProblem::Denied => "denied",
                WriteProblem::Other => "other",
            };
            format!("write:{problem}:{path}")
        }
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
        "off" => ReadingFailure::SourceOff { source: rest.to_owned() },
        "sign_in" => ReadingFailure::SignInNeeded { what: rest.to_owned() },
        "blocked" => {
            let (source, reason) = rest.split_once(':').unwrap_or((rest, ""));
            ReadingFailure::Blocked { source: source.to_owned(), reason: reason.to_owned() }
        }
        "file" => ReadingFailure::FileMissing { path: rest.to_owned() },
        "write" => {
            let (problem, path) = rest.split_once(':').unwrap_or(("other", rest));
            let problem = match problem {
                "name" => WriteProblem::Name,
                "denied" => WriteProblem::Denied,
                _ => WriteProblem::Other,
            };
            ReadingFailure::CannotWrite { path: path.to_owned(), problem }
        }
        _ => ReadingFailure::SourceDown { source: rest.to_owned() },
    }
}

/// A work's chapters in reading order, and the source they come from.
/// The Downloads finished notification for what a run finished: one series by name, in its own
/// unit, or several counted.
fn done_notice(
    store: &Store,
    finished: &[(String, usize)],
    form: kaeri_store::shelf::TitleForm,
) -> ReadingEvent {
    let title = Msg::Reading(ReadingMsg::ReadingDownloadsDoneTitle);
    let count = |n: usize| i64::try_from(n).unwrap_or(i64::MAX);
    match finished {
        [(work, n)] => {
            let kind = store.book_work_kind(work).ok().flatten();
            let novel = kind.as_ref().is_some_and(|k| k.medium == "novel");
            let name = kind.and_then(|k| k.title_in(form)).unwrap_or_else(|| work.clone());
            let body = if novel {
                ReadingMsg::ReadingDownloadsDoneParts { title: name, count: count(*n) }
            } else {
                ReadingMsg::ReadingDownloadsDoneChapters { title: name, count: count(*n) }
            };
            ReadingEvent::DownloadsDone {
                work: Some(WorkId::new(work.clone())),
                title,
                body: Msg::Reading(body),
            }
        }
        _ => ReadingEvent::DownloadsDone {
            work: None,
            title,
            body: Msg::Reading(ReadingMsg::ReadingDownloadsDoneSeries {
                count: count(finished.len()),
            }),
        },
    }
}

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
    // A chapter listed from another source is downloaded from there: see [`Listers`].
    Ok((source, flatten(groups)))
}

/// Where each chapter of a work is downloaded from: the edition's source, or for a chapter the
/// series page lists from another source, the first other source whose kept list has it, as
/// the series page fills it in and the reader opens it.
pub(crate) struct Listers {
    edition: String,
    /// Each other source with the chapter ids its kept list has, in the work's order.
    others: Vec<(String, HashSet<String>)>,
}

impl Listers {
    pub(crate) fn of(sources: &BookSources, work: &WorkId, edition: &str) -> Self {
        let store = sources.store();
        let others = store
            .book_refs_for_work(work.as_str())
            .unwrap_or_default()
            .into_iter()
            .filter(|(other, _)| other != edition && sources.is_available(other))
            .filter_map(|(other, series)| {
                let (list, _) = crate::read_catalog::chapters::kept(store, &other, &series)?;
                Some((other, list.into_iter().map(|k| k.id).collect()))
            })
            .collect();
        Self { edition: edition.to_owned(), others }
    }

    /// The source `row` is downloaded from. `None` for a chapter listed from another source
    /// that no source's kept list has any more.
    pub(crate) fn of_row(&self, row: &ChapterRow) -> Option<&str> {
        if row.other_source.is_none() {
            return Some(&self.edition);
        }
        self.others
            .iter()
            .find(|(_, ids)| ids.contains(row.id.as_str()))
            .map(|(s, _)| s.as_str())
    }
}

/// The reader's place in `rows`, in reading order: the last chapter in progress, or the one
/// after the last read, and the start when nothing was read. Choose chapters starts there too.
fn place(rows: &[ChapterRow]) -> usize {
    if let Some(at) =
        rows.iter().rposition(|r| matches!(r.state, ChapterReadState::InProgress { .. }))
    {
        return at;
    }
    rows.iter().rposition(|r| matches!(r.state, ChapterReadState::Read)).map_or(0, |at| at + 1)
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
        engine.said_network(),
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

mod kept;
mod shared;
mod sizes;
#[cfg(test)]
mod tests;

use kept::{KeptPages, pages_of};
pub(crate) use shared::SharedPages;
