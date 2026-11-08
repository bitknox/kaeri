//! Updates: the chapters that came out in the series the person follows, by day, and the paced
//! check that asks each series' source for its list again.
//!
//! The check keeps every answer in the chapter cache, learns each series' rhythm from it, and
//! tells of a chapter only once per profile (`read_notice`), only while reading is on and the
//! chapters notification is. The apps post the notification; the engine only says what is new.

use std::{
    collections::{HashMap, HashSet},
    sync::atomic::Ordering,
};

use kaeri_core::{
    books::{BookHints, ChapterId, WorkId},
    config::NewChapters,
};
use kaeri_model::reading::{
    ChapterReadState, ChapterRow, NewChapter, ReadingEvent, UpdateDay, UpdateFailure,
    UpdateRow, UpdatesView, WorkTile,
};
use kaeri_store::Store;

use super::{Ask, Kit, chapters, send, tiles};
use crate::{
    downloads::books::{Lent, Wanted},
    outbox::Outbox,
    read_sources::BookSources,
};

/// How far back Updates goes.
const UPDATES_FOR: i64 = 30 * 86_400;
const CHECKED: &str = "read:updates:checked";

/// The series Updates covers: Continue's and those on a list still being read.
pub(crate) fn followed(store: &Store) -> Vec<String> {
    let statuses: HashMap<String, String> =
        store.read_statuses().unwrap_or_default().into_iter().collect();
    let mut out: Vec<String> = store
        .continue_reading(200)
        .unwrap_or_default()
        .into_iter()
        .map(|w| w.work)
        .filter(|w| {
            !matches!(statuses.get(w).map(String::as_str), Some("completed" | "dropped"))
        })
        .collect();
    for (work, status) in &statuses {
        if matches!(status.as_str(), "reading" | "rereading" | "paused") && !out.contains(work)
        {
            out.push(work.clone());
        }
    }
    out
}

fn checked_at(store: &Store) -> Option<i64> {
    store.get_meta_text(CHECKED).ok()??.parse().ok()
}

/// Updates as kept, at once; then again once the books of series nothing is kept of land, so
/// a row opened first in a fresh process, as from a notification, gets its cover and medium.
pub fn load(kit: &Kit, ask: Ask, tx: &Outbox) {
    send(tx, ReadingEvent::Updates(view(kit, ask)));
    let unkept = tiles::unkept(&kit.meta, &followed(&kit.store));
    if unkept.is_empty() {
        return;
    }
    let (kit, tx) = (kit.clone(), tx.clone());
    tokio::spawn(async move {
        match kaeri_meta::on_screen(kit.meta.books_many(&unkept)).await {
            Ok(_) => send(&tx, ReadingEvent::Updates(view(&kit, ask))),
            Err(e) => tracing::info!(error = %e, "the books behind Updates were not asked for"),
        }
    });
}

pub(crate) fn view(kit: &Kit, ask: Ask) -> UpdatesView {
    let store = &kit.store;
    let sources = &kit.sources;
    let zone = kit.clock.zone();
    let since = ask.now - UPDATES_FOR;
    let mut rows: Vec<(i64, UpdateRow, bool)> = Vec::new();
    for work in followed(store) {
        let id = WorkId::new(work.clone());
        let Some((source, _, list)) = chapters::of_work(sources, store, &id) else { continue };
        let read = chapters::Read::of(store, &work);
        // One row per chapter: two versions of it are one chapter come out.
        let mut listed = HashSet::new();
        let new: Vec<&chapters::Kept> = list
            .iter()
            .filter(|c| listed.insert(c.id.as_str()))
            .filter(|c| c.seen_at.is_some() || c.published.is_some())
            .filter(|c| c.out_at().is_some_and(|at| at >= since && at <= ask.now + 86_400))
            .collect();
        if new.is_empty() {
            continue;
        }
        let Some(tile) = tile_of(kit, &work) else { continue };
        let source_name = source_name(sources, &source);
        for chapter in new {
            let is_read = read.is_read(&chapter.id);
            let mut chapter_row = row_of(chapter, &read);
            chapter_row.line = super::rows::line(
                &chapter_row,
                super::rows::Kept::Source,
                tile.medium,
                ask.now,
                &zone,
            );
            let row = UpdateRow {
                tile: tile.clone(),
                chapter: chapter_row,
                source_name: source_name.clone(),
            };
            rows.push((chapter.out_at().unwrap_or(ask.now), row, is_read));
        }
    }
    rows.sort_by_key(|r| std::cmp::Reverse(r.0));
    let unread = u32::try_from(rows.iter().filter(|r| !r.2).count()).unwrap_or(u32::MAX);
    let mut days: Vec<UpdateDay> = Vec::new();
    for (at, row, _) in rows {
        let day = chapters::day_start(at, &zone).unwrap_or(at);
        match days.last_mut() {
            Some(last) if last.day == day => last.rows.push(row),
            _ => days.push(UpdateDay { day, rows: vec![row] }),
        }
    }
    UpdatesView {
        days,
        checked_at: checked_at(store),
        failures: sources.catalog.failures.lock().unwrap_or_else(|e| e.into_inner()).clone(),
        checking: sources.catalog.checking.load(Ordering::Relaxed),
        unread,
    }
}

fn tile_of(kit: &Kit, work: &str) -> Option<WorkTile> {
    if let Some(kept) = tiles::anilist(work).and_then(|id| kit.meta.kept_book(id)) {
        return Some(tiles::reading(&kit.meta, &kit.store, &kit.sources, &kept.book));
    }
    if let Some(tile) = tiles::demo(work, &kit.store, &kit.sources) {
        return Some(tile);
    }
    // Nothing kept of it yet: its folder's name, or its key until the catalogue answers.
    let local = kit.store.local_book_series().unwrap_or_default();
    let series = local.iter().find(|s| s.work.as_deref() == Some(work));
    let medium = series
        .and_then(|s| kaeri_core::media::Medium::parse(&s.medium))
        .unwrap_or(kaeri_core::media::Medium::Manga);
    let mut tile = WorkTile {
        work: WorkId::new(work.to_owned()),
        title: series.map_or_else(|| work.to_owned(), |s| s.name.clone()),
        medium,
        cover: None,
        format_label: None,
        progress: None,
        next: None,
        new_chapter: false,
        adult: false,
        facts: Vec::new(),
        demo: false,
        not_in_sources: false,
        origin: None,
        amount: None,
        started: None,
    };
    tiles::mark(&mut tile, &kit.store, &kit.sources);
    Some(tile)
}

fn source_name(sources: &BookSources, source: &str) -> String {
    sources.registry.manifest(source).map_or_else(|| source.to_owned(), |m| m.name)
}

pub(crate) fn row_of(chapter: &chapters::Kept, read: &chapters::Read) -> ChapterRow {
    let state = if read.is_read(&chapter.id) {
        ChapterReadState::Read
    } else {
        match &read.in_progress {
            Some((id, Some(page), of)) if *id == chapter.id => ChapterReadState::InProgress {
                page: kaeri_model::reading::shown_page(*page),
                of: *of,
            },
            _ => ChapterReadState::Unread,
        }
    };
    ChapterRow {
        id: ChapterId::new(chapter.id.clone()),
        label: chapter.label.clone(),
        state,
        number: chapter.number.clone(),
        title: chapter.own_title().map(str::to_owned),
        volume: chapter.volume.clone(),
        published: chapter.published,
        pages: chapter.pages,
        new_chapter: !matches!(state, ChapterReadState::Read),
        download: None,
        versions: 1,
        bookmarked: false,
        version_label: chapter.version_label.clone(),
        read_at: None,
        line: Vec::new(),
        gone_from_source: false,
    }
}

/// Run the check: each followed series' source is asked once, one after another. A check
/// already running is joined, not started again.
pub fn check(kit: &Kit, ask: Ask, tx: &Outbox) {
    let flag = kit.sources.catalog.checking.clone();
    if flag.swap(true, Ordering::AcqRel) {
        return load(kit, ask, tx);
    }
    load(kit, ask, tx);
    let kit = kit.clone();
    let tx = tx.clone();
    tokio::spawn(async move {
        let turn = kit.sources.catalog.check_turn.clone();
        let _turn = turn.lock().await;
        let checked = run(&kit, ask).await;
        flag.store(false, Ordering::Release);
        load(&kit, ask, &tx);
        if checked.queued > 0 {
            let lent = Lent {
                sources: kit.sources.clone(),
                plugins: kit.plugins.clone(),
                clock: kit.clock.clone(),
                tx: tx.clone(),
            };
            kit.sources.downloads.publish(&kit.store, &tx);
            kit.sources.downloads.start(&lent);
        }
        if !checked.found.is_empty() {
            send(&tx, ReadingEvent::NewChapters(checked.found));
        }
    });
}

/// What a check found.
#[derive(Debug, Default)]
pub(crate) struct Checked {
    /// The chapters to tell of.
    pub found: Vec<NewChapter>,
    /// How many new chapters went into the download queue.
    pub queued: usize,
}

/// Ask every followed series' source again, give the chapters to tell of, and queue the new
/// chapters the New chapters setting downloads.
pub(crate) async fn run(kit: &Kit, ask: Ask) -> Checked {
    let (store, sources) = (&kit.store, &kit.sources);
    sources.adopt_plugins(&kit.plugins);
    let zone = kit.clock.zone();
    let tell = ask.chapter_notices && ask.answer.reads();
    let download = sources.downloads.setup().new_chapters == NewChapters::Reading;
    let statuses: HashMap<String, String> = if download {
        store.read_statuses().unwrap_or_default().into_iter().collect()
    } else {
        HashMap::new()
    };
    let mut queued = 0;
    let mut found = Vec::new();
    let mut failures = Vec::new();
    let mut asked = HashSet::new();
    for work in followed(store) {
        let id = WorkId::new(work.clone());
        let Some((source, series)) = sources.edition_of(&id, None) else { continue };
        if !asked.insert((source.clone(), series.clone())) {
            continue;
        }
        let listed = match sources
            .registry
            .chapters(&source, &series, &BookHints::default(), ask.now)
            .await
        {
            Ok(listed) => listed,
            Err(e) => {
                tracing::info!(error = %e, "a followed series' source did not answer");
                let title = tile_of(kit, &work).map_or_else(|| work.clone(), |t| t.title);
                failures.push(UpdateFailure {
                    work: id,
                    title,
                    source_name: source_name(sources, &source),
                    failure: crate::read_sources::failure(&source, &e),
                });
                continue;
            }
        };
        let fresh = chapters::keep(store, &source, &series, &listed, ask.now);
        let Some((list, _)) = chapters::kept(store, &source, &series) else { continue };
        if let Some(rhythm) = chapters::rhythm(&list, &zone, ask.now)
            && let Err(e) = store.set_book_rhythm(&work, &rhythm)
        {
            tracing::warn!(error = %e, "a series' rhythm was not kept");
        }
        if fresh.is_empty() {
            continue;
        }
        let read = chapters::Read::of(store, &work);
        // New chapters of a series on the Reading list download when the setting says so,
        // whether or not anyone is told of them.
        if download && statuses.get(&work).map(String::as_str) == Some("reading") {
            let wanted: Vec<Wanted> = fresh
                .iter()
                .filter(|c| !read.is_read(&c.id))
                .map(|c| Wanted {
                    chapter: ChapterId::new(c.id.clone()),
                    version_label: c.version_label.clone(),
                })
                .collect();
            queued += sources.downloads.queue(store, &id, &source, &wanted, true, ask.now);
        }
        if !tell {
            continue;
        }
        // Only a series the person reads: one never opened is not followed for its news.
        if !read.any() {
            continue;
        }
        let kept = tiles::anilist(&work).and_then(|a| kit.meta.kept_book(a));
        let next = read.next(&list).map(|c| ChapterId::new(c.id.clone()));
        for chapter in fresh.iter().filter(|c| !read.is_read(&c.id)) {
            match store.notice_chapter(&work, &chapter.id, ask.now) {
                Ok(true) => {}
                Ok(false) => continue,
                Err(e) => {
                    tracing::warn!(error = %e, "a chapter notice was not kept");
                    continue;
                }
            }
            let title = kept.as_ref().map_or_else(
                || work.clone(),
                |k| kit.meta.titles().pick(&k.book.title).to_owned(),
            );
            found.push(NewChapter {
                work: id.clone(),
                title,
                chapter: ChapterId::new(chapter.id.clone()),
                label: chapter.label.clone(),
                cover: kept.as_ref().and_then(|k| k.book.cover_image.best().map(str::to_owned)),
                banner: kept.as_ref().and_then(|k| k.book.banner_image.clone()),
                published: chapter.published,
                next_for_you: next.clone().filter(|n| n.as_str() != chapter.id),
                source_name: Some(source_name(sources, &source)),
                chapter_title: chapter.own_title().map(str::to_owned),
            });
        }
    }
    if let Err(e) = store.set_meta_text(CHECKED, &ask.now.to_string()) {
        tracing::warn!(error = %e, "the check's time was not kept");
    }
    *sources.catalog.failures.lock().unwrap_or_else(|e| e.into_inner()) = failures;
    Checked { found, queued }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex, atomic::AtomicBool};

    use async_trait::async_trait;
    use kaeri_core::{
        ProviderError,
        books::{
            BookSourceKind, BookSourceManifest, ChapterContent, SeriesDetail, SeriesHit,
            SourceChapter,
        },
        config::Paths,
        media::Medium,
        traits::BookSource,
    };
    use kaeri_model::medium::KaeriIsFor;

    use super::*;

    /// A source whose list a test grows.
    struct Shelf {
        manifest: BookSourceManifest,
        listed: Mutex<Vec<SourceChapter>>,
    }

    #[async_trait]
    impl BookSource for Shelf {
        fn manifest(&self) -> &BookSourceManifest {
            &self.manifest
        }

        async fn search(
            &self,
            _query: &str,
            _hints: &BookHints,
        ) -> Result<Vec<SeriesHit>, ProviderError> {
            Ok(Vec::new())
        }

        async fn series(
            &self,
            id: &str,
            _hints: &BookHints,
        ) -> Result<SeriesDetail, ProviderError> {
            Ok(SeriesDetail::new(id, "The Lantern Ferry"))
        }

        async fn chapters(
            &self,
            _id: &str,
            _hints: &BookHints,
        ) -> Result<Vec<SourceChapter>, ProviderError> {
            Ok(self.listed.lock().unwrap().clone())
        }

        async fn content(
            &self,
            _series: &str,
            _chapter: &str,
        ) -> Result<ChapterContent, ProviderError> {
            Err(ProviderError::NotFound)
        }
    }

    fn chapter(n: u32) -> SourceChapter {
        SourceChapter::new(format!("c-{n}"), n.to_string())
    }

    fn kit(dir: &std::path::Path, shelf: Arc<Shelf>) -> Kit {
        let store = Store::open_in_memory().unwrap();
        let paths = Paths {
            config_file: dir.join("config.toml"),
            data_dir: dir.join("data"),
            cache_dir: dir.join("cache"),
        };
        let tokens = kaeri_track::TokenStore::new(dir.join("tokens"));
        let sources = BookSources::new(store.clone(), tokens, reqwest::Client::new(), &paths);
        sources.registry.put(shelf);
        let http =
            kaeri_net::HttpClient::new(&kaeri_core::config::NetworkConfig::default()).unwrap();
        let meta =
            kaeri_meta::Meta::new(kaeri_meta::AniList::new(http, 30), None, store.clone());
        Kit {
            meta,
            store,
            sources,
            plugins: crate::book_plugins::BookPlugins::default(),
            clock: crate::clock::Clock::default(),
            offline: Arc::new(AtomicBool::new(false)),
        }
    }

    fn ask(now: i64, notices: bool) -> Ask {
        Ask { answer: KaeriIsFor::Both, adult: false, chapter_notices: notices, now }
    }

    /// A source that once answers an empty list (a server part way through a scan, a plugin
    /// whose scrape broke) and then the full list again: nothing came out, so nothing is told,
    /// queued to download or shown as new, and a list kept empty by an earlier version counts
    /// as no list.
    #[tokio::test]
    async fn an_empty_answer_makes_nothing_new() {
        use crate::downloads::books::Setup;
        let dir = tempfile::tempdir().unwrap();
        let manifest = BookSourceManifest {
            id: "shelf".into(),
            name: "Shelf".into(),
            version: "1".into(),
            kind: BookSourceKind::Native,
            media: vec![Medium::Manga],
            languages: Vec::new(),
            allowed_hosts: Vec::new(),
            capabilities: Vec::new(),
        };
        let full: Vec<SourceChapter> = (1..=60).map(chapter).collect();
        let shelf = Arc::new(Shelf { manifest, listed: Mutex::new(full.clone()) });
        let kit = kit(dir.path(), shelf.clone());
        let work = "a:900301";
        kit.store.set_book_match("shelf", "s-1", work, 100, true, false, 1).unwrap();
        kit.store.adopt_read(work, "10", None, 2).unwrap();
        kit.store.set_read_status(work, Some("reading"), 2).unwrap();
        let setup = Setup {
            keep_ready: true,
            new_chapters: NewChapters::Reading,
            hold: true,
            ..Setup::default()
        };
        kit.sources.downloads.configure(setup);
        let rows = |now: i64| -> usize {
            view(&kit, ask(now, true)).days.iter().map(|d| d.rows.len()).sum()
        };
        let first = run(&kit, ask(1_000, true)).await;
        assert!(first.found.is_empty() && first.queued == 0);
        assert_eq!(rows(1_000), 0);

        *shelf.listed.lock().unwrap() = Vec::new();
        let empty = run(&kit, ask(2_000, true)).await;
        assert!(empty.found.is_empty() && empty.queued == 0);
        assert_eq!(chapters::kept(&kit.store, "shelf", "s-1").unwrap().0.len(), 60);

        *shelf.listed.lock().unwrap() = full.clone();
        let again = run(&kit, ask(3_000, true)).await;
        assert!(again.found.is_empty(), "nothing came out: {} told", again.found.len());
        assert_eq!(again.queued, 0, "nothing came out to download");
        assert_eq!(rows(3_000), 0, "Updates shows nothing new");

        // A list an earlier version kept empty is no list: the next answer is a first one.
        kit.store.put_book_chapters("shelf", "s-1", "[]", 3_500).unwrap();
        let after = run(&kit, ask(4_000, true)).await;
        assert!(after.found.is_empty() && after.queued == 0);
        assert_eq!(rows(4_000), 0);
    }

    #[tokio::test]
    async fn a_new_chapter_is_told_once_and_only_with_the_switch_on() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = BookSourceManifest {
            id: "shelf".into(),
            name: "Shelf".into(),
            version: "1".into(),
            kind: BookSourceKind::Native,
            media: vec![Medium::Manga],
            languages: vec!["en".into()],
            allowed_hosts: Vec::new(),
            capabilities: Vec::new(),
        };
        let shelf =
            Arc::new(Shelf { manifest, listed: Mutex::new((1..=12).map(chapter).collect()) });
        let kit = kit(dir.path(), shelf.clone());
        let work = "a:900301";
        kit.store.set_book_match("shelf", "s-1", work, 100, true, false, 1).unwrap();
        kit.store.adopt_read(work, "12", None, 2).unwrap();
        kit.store.set_read_status(work, Some("reading"), 2).unwrap();

        // The first answer is what was out before: nothing to tell.
        assert!(run(&kit, ask(1_000, true)).await.found.is_empty());
        shelf.listed.lock().unwrap().push(chapter(13));
        // With the switch off the chapter is kept, and nobody is told.
        assert!(run(&kit, ask(2_000, false)).await.found.is_empty());
        shelf.listed.lock().unwrap().push(chapter(14));
        let found = run(&kit, ask(3_000, true)).await.found;
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].chapter.as_str(), "14");
        assert_eq!(found[0].next_for_you.as_ref().map(ChapterId::as_str), Some("13"));
        assert_eq!(found[0].source_name.as_deref(), Some("Shelf"));
        // Asked again, the same chapter is not told twice.
        assert!(run(&kit, ask(4_000, true)).await.found.is_empty());

        // Updates lists both new chapters, newest first, neither read.
        let view = view(&kit, ask(4_000, true));
        assert_eq!(view.checked_at, Some(4_000));
        let rows: Vec<&str> =
            view.days.iter().flat_map(|d| &d.rows).map(|r| r.chapter.id.as_str()).collect();
        assert_eq!(rows, ["14", "13"]);
        assert_eq!(view.unread, 2);
    }

    #[tokio::test]
    async fn new_chapters_of_series_on_reading_download_when_the_setting_says_so() {
        use crate::downloads::books::Setup;
        let dir = tempfile::tempdir().unwrap();
        let manifest = BookSourceManifest {
            id: "shelf".into(),
            name: "Shelf".into(),
            version: "1".into(),
            kind: BookSourceKind::Native,
            media: vec![Medium::Manga],
            languages: Vec::new(),
            allowed_hosts: Vec::new(),
            capabilities: Vec::new(),
        };
        let shelf =
            Arc::new(Shelf { manifest, listed: Mutex::new((1..=5).map(chapter).collect()) });
        let kit = kit(dir.path(), shelf.clone());
        let (reading, paused) = ("a:900311", "a:900312");
        for (work, series, status) in [(reading, "s-1", "reading"), (paused, "s-2", "paused")] {
            kit.store.set_book_match("shelf", series, work, 100, true, false, 1).unwrap();
            kit.store.adopt_read(work, "5", None, 2).unwrap();
            kit.store.set_read_status(work, Some(status), 2).unwrap();
        }
        let queued = |work: &str| -> Vec<String> {
            kit.store
                .book_downloads(Some(work))
                .unwrap()
                .into_iter()
                .map(|d| d.chapter)
                .collect()
        };

        // Off: a new chapter is told of and nothing downloads.
        assert_eq!(run(&kit, ask(1_000, false)).await.queued, 0);
        shelf.listed.lock().unwrap().push(chapter(6));
        assert_eq!(run(&kit, ask(2_000, false)).await.queued, 0);
        assert!(queued(reading).is_empty());

        // Series on Reading: the Reading list's new chapter queues, kept ready so it goes once
        // read, with notices off too. The paused series' does not.
        let setup = Setup {
            keep_ready: true,
            new_chapters: NewChapters::Reading,
            hold: true,
            ..Setup::default()
        };
        kit.sources.downloads.configure(setup);
        shelf.listed.lock().unwrap().push(chapter(7));
        assert_eq!(run(&kit, ask(3_000, false)).await.queued, 1);
        assert_eq!(queued(reading), ["7"]);
        assert!(queued(paused).is_empty());
        assert!(kit.store.book_downloads(Some(reading)).unwrap()[0].keep_ready);
        // Asked again, nothing new queues.
        assert_eq!(run(&kit, ask(4_000, false)).await.queued, 0);
    }

    #[tokio::test]
    async fn someone_who_only_watches_is_told_of_no_chapters() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = BookSourceManifest {
            id: "shelf".into(),
            name: "Shelf".into(),
            version: "1".into(),
            kind: BookSourceKind::Native,
            media: vec![Medium::Manga],
            languages: Vec::new(),
            allowed_hosts: Vec::new(),
            capabilities: Vec::new(),
        };
        let shelf = Arc::new(Shelf { manifest, listed: Mutex::new(vec![chapter(1)]) });
        let kit = kit(dir.path(), shelf.clone());
        kit.store.set_book_match("shelf", "s-1", "a:900301", 100, true, false, 1).unwrap();
        kit.store.adopt_read("a:900301", "1", None, 2).unwrap();
        kit.store.set_read_status("a:900301", Some("reading"), 2).unwrap();
        run(&kit, ask(1_000, true)).await;
        shelf.listed.lock().unwrap().push(chapter(2));
        let watch = Ask { answer: KaeriIsFor::Watch, ..ask(2_000, true) };
        assert!(run(&kit, watch).await.found.is_empty());
        // The chapter was kept all the same: Updates has it.
        let rows =
            view(&kit, ask(2_000, true)).days.iter().map(|d| d.rows.len()).sum::<usize>();
        assert_eq!(rows, 1);
    }
}
