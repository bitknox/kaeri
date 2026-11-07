//! The reading scenes' history, books and catalogue: the same made-up works the phones' Fixtures
//! builds read, written into the scene's store so the Read places, a series page and the reader
//! have something to show. Nothing here names a real work.

use kaeri_store::Store;

use kaeri_testkit::books as scene_books;

/// The works the reading scenes use, made up like every fixture.
pub const READING_MANGA: &str = "a:900301";
pub const READING_NOVEL: &str = "a:900302";
pub const READING_STRIP: &str = "a:900303";

/// The works' series as their generated books are kept.
pub const MANGA_SERIES: scene_books::Series = scene_books::Series {
    id: "fixture-lantern-ferry",
    work: READING_MANGA,
    name: "The Lantern Ferry",
    medium: "manga",
    format: "MANGA",
    origin: Some("JP"),
};
pub const NOVEL_SERIES: scene_books::Series = scene_books::Series {
    id: "fixture-glass-orchard",
    work: READING_NOVEL,
    name: "The Glass Orchard",
    medium: "novel",
    format: "NOVEL",
    origin: Some("JP"),
};
pub const STRIP_SERIES: scene_books::Series = scene_books::Series {
    id: "fixture-paper-kites",
    work: READING_STRIP,
    name: "Paper Kites Over Juniper Row",
    medium: "manga",
    format: "MANGA",
    origin: Some("KR"),
};

/// The books the reading scenes read, made on the spot: the manga's chapters 12 to 15 of twenty
/// pages, the strip's three chapters of eight tall images and the novel's chapters 2 to 4.
pub fn seed_books(store: &Store, data_dir: &std::path::Path, now: i64) {
    scene_books::comics(store, data_dir, &MANGA_SERIES, &[12, 13, 14, 15], 20, now);
    scene_books::strips(store, data_dir, &STRIP_SERIES, &[1, 2, 3], 8, now);
    scene_books::novels(store, data_dir, &NOVEL_SERIES, &[2, 3, 4], now);
    // The folder Your sources lists, holding the three series.
    let root = data_dir.join("fixture-books");
    let folder = kaeri_store::book_sources::Folder {
        id: BOOKS_FOLDER.into(),
        path: Some(root.display().to_string()),
        tree_uri: None,
        bookmark: None,
        name: Some("Books".into()), // i18n-skip
        added_at: now - 40 * 24 * 3_600,
        last_scan: Some(now - 3_600),
        available: true,
    };
    if let Err(e) = store.put_book_folder(&folder) {
        tracing::warn!(error = %e, "a reading scene's folder was not seeded");
    }
    for series in [&MANGA_SERIES, &STRIP_SERIES, &NOVEL_SERIES] {
        let local = kaeri_store::shelf::LocalSeries {
            id: series.id.into(),
            folder: BOOKS_FOLDER.into(),
            name: series.name.into(),
            medium: series.medium.into(),
            work: Some(series.work.into()),
            state: "matched".into(),
        };
        if let Err(e) = store.put_local_series(&local) {
            tracing::warn!(error = %e, "a reading scene's series was not put in its folder");
        }
    }
}

/// The id of the folder the reading scene's books sit in.
const BOOKS_FOLDER: &str = "fixture-books-folder";

/// Set once a home has its reading scene, so a later start on the same home keeps what the
/// person changed since: the reader's settings for a series and the places reached.
const READING_SEEDED: &str = "fixtures:reading:seeded";

/// The reading history of the reading scene, written on a home's first start
/// only. The Fixtures apps start each launch in an empty home, so a launch shows the scene as
/// it is drawn; an engine started again on the same home, after a restart or on a kept home,
/// goes on from where the person left it.
pub fn seed_reading(store: &Store, downloads: bool, now: i64) {
    use kaeri_store::reading::ReadEvent;
    const DAY: i64 = 24 * 3_600;
    if store.get_meta_i64(READING_SEEDED).ok().flatten().is_some() {
        return;
    }
    if let Err(e) = store.set_meta_i64(READING_SEEDED, now) {
        tracing::warn!(error = %e, "a reading scene's home was not marked as seeded");
    }
    let read = |work: &str, chapter: &str, at: i64, fill: &dyn Fn(&mut ReadEvent)| {
        let mut event = ReadEvent::new(work, chapter, at);
        fill(&mut event);
        if let Err(e) = store.record_read(&event, Vec::new()) {
            tracing::warn!(error = %e, "a reading scene's read was not seeded");
        }
    };
    let _ = store.adopt_read(READING_MANGA, "12", None, now - 3 * DAY);
    read(READING_MANGA, "13", now - 2 * 3_600, &|e| {
        e.page = Some(9);
        e.of_pages = Some(20);
        e.source_id = Some("fixture-books".into());
    });
    read(READING_NOVEL, "3", now - DAY, &|e| {
        e.novel_pos = Some("3/0.4.1/120".into());
        // The reader keeps the page it laid out with the place: a third of the way in.
        e.page = Some(16);
        e.of_pages = Some(47);
    });
    seed_reading_catalog(store, now);
    let _ = store.set_series_setting(READING_MANGA, "direction", Some("rtl"), true, now);
    let _ = store.set_bookmark(READING_MANGA, "7", true, now - DAY);
    let _ = store.set_series_setting(READING_STRIP, "mode", Some("strip"), true, now);
    // Chapter 1 finished, chapter 2 under way.
    let _ = store.adopt_read(READING_STRIP, "1", None, now - 2 * DAY);
    // As the reader writes a strip's place: the image as the page, and how far into it.
    read(READING_STRIP, "2", now - 600, &|e| {
        e.page = Some(4);
        e.strip_image = Some(4);
        e.strip_offset = Some(0.35);
    });
    if downloads {
        seed_book_downloads(store, now);
    }
}

/// The queue of the downloads scene, written as the engine writes it. The files are
/// not there: the Downloads screen shows rows, and nothing opens them.
fn seed_book_downloads(store: &Store, now: i64) {
    use kaeri_store::book_downloads::BookDownloadAsk;
    let ask = |work: &'static str, chapter: &'static str, keep_ready: bool| {
        let ask = BookDownloadAsk {
            work,
            chapter,
            source_id: "fixture-books",
            version_label: None,
            keep_ready,
        };
        if let Err(e) = store.queue_book_download(&ask, now - 600) {
            tracing::warn!(error = %e, "a reading scene's download was not seeded");
        }
        store.book_download(work, chapter).ok().flatten().map(|row| row.id)
    };
    let warn = |e: kaeri_store::StoreError| {
        tracing::warn!(error = %e, "a reading scene's download was not seeded");
    };
    if let Some(id) = ask(READING_MANGA, "14", false) {
        let path = "/data/book-downloads/a~3a900301/14.cbz";
        store.book_download_done(id, path, Some(20), 9_400_000, now - 500).unwrap_or_else(warn);
    }
    if let Some(id) = ask(READING_MANGA, "15", true) {
        store.book_download_progress(id, 6, Some(20), now - 60).unwrap_or_else(warn);
    }
    ask(READING_MANGA, "16", true);
    ask(READING_MANGA, "17", true);
    if let Some(id) = ask(READING_NOVEL, "4", false) {
        let path = "/data/book-downloads/a~3a900302/4.epub";
        store.book_download_done(id, path, None, 1_200_000, now - 400).unwrap_or_else(warn);
    }
    if let Some(id) = ask(READING_STRIP, "2", false) {
        store.book_download_failed(id, "offline", now - 300).unwrap_or_else(warn);
    }
}

/// What reading's catalogue kept for the reading scene: which folder series
/// each work is, the chapters each listed last, how often they come out, and the reading lists.
/// The books themselves come from the fake AniList.
fn seed_reading_catalog(store: &Store, now: i64) {
    use kaeri_store::book_catalog::BookRhythm;
    const DAY: i64 = 24 * 3_600;
    let warn = |e: kaeri_store::StoreError| {
        tracing::warn!(error = %e, "a reading scene's catalogue was not seeded");
    };
    // In the chapter cache's own JSON, which the engine reads back.
    let chapter = |n: i64, published: i64, seen_at: Option<i64>, title: Option<&str>| {
        let optional = |v: Option<String>| v.unwrap_or_else(|| "null".into());
        format!(
            r#"{{"id":"{n}","source_ref":"c-{n}","label":"Chapter {n}","number":"{n}","title":{},"language":"en","published":{published},"seen_at":{}}}"#,
            optional(title.map(|t| format!("\"{t}\""))),
            optional(seen_at.map(|at| at.to_string())),
        )
    };
    // The manga comes out weekly; chapter 15 two hours ago, seen on the last check.
    let manga: Vec<_> = (1..=15)
        .map(|n| {
            let newest = n == 15;
            let seen = newest.then_some(now - 3_600);
            chapter(
                n,
                now - 2 * 3_600 - (15 - n) * 7 * DAY,
                seen,
                newest.then_some("The Far Shore"),
            )
        })
        .collect();
    // The novel, a part a month.
    let novel: Vec<_> =
        (1..=5).map(|n| chapter(n, now - DAY - (5 - n) * 30 * DAY, None, None)).collect();
    // The strip, weekly, the newest three days ago.
    let strip: Vec<_> =
        (1..=8).map(|n| chapter(n, now - 3 * DAY - (8 - n) * 7 * DAY, None, None)).collect();
    for (work, series, list) in [
        (READING_MANGA, MANGA_SERIES.id, manga),
        (READING_NOVEL, NOVEL_SERIES.id, novel),
        (READING_STRIP, STRIP_SERIES.id, strip),
    ] {
        let _ = store
            .set_book_match("folders", series, work, 100, true, false, now - 40 * DAY)
            .map_err(warn);
        let json = format!("[{}]", list.join(","));
        let _ = store.put_book_chapters("folders", series, &json, now - 3_600).map_err(warn);
    }
    for (work, last) in [(READING_MANGA, now - 2 * 3_600), (READING_STRIP, now - 3 * DAY)] {
        let rhythm = BookRhythm {
            weekday: None,
            every_days: Some(7),
            last_published: Some(last),
            checked_at: now - 3_600,
        };
        let _ = store.set_book_rhythm(work, &rhythm).map_err(warn);
    }
    for (work, status) in [
        (READING_MANGA, "reading"),
        (READING_STRIP, "reading"),
        ("a:900304", "planning"),
        ("a:900306", "planning"),
        ("a:900307", "completed"),
    ] {
        let _ = store.set_read_status(work, Some(status), now - DAY).map_err(warn);
    }
    let _ =
        store.set_meta_text("read:updates:checked", &(now - 3_600).to_string()).map_err(warn);
}

/// The three works as the catalogue keeps them once fetched, covers included, so Updates and
/// Downloads, which name a series from what is kept, have its art from the start. The art points
/// at `art`, where the fake serves it, as the fake's own answers do; on every start, as its port
/// changes.
pub fn keep_books(store: &Store, art: &str, now: i64) {
    use kaeri_meta::anilist::books::Book;
    use kaeri_store::book_catalog::BookWork;
    for value in kaeri_testkit::fixtures::books() {
        let json = value.to_string().replace(kaeri_testkit::fixtures::ART_HOST, art);
        let Ok(book) = serde_json::from_str::<Book>(&json) else { continue };
        let work = format!("a:{}", book.id.get());
        if ![READING_MANGA, READING_NOVEL, READING_STRIP].contains(&work.as_str()) {
            continue;
        }
        let titles = serde_json::json!({
            "romaji": book.title.romaji,
            "english": book.title.english,
            "native": book.title.native,
            "synonyms": book.synonyms,
        });
        let status = book
            .status
            .and_then(|s| serde_json::to_value(s).ok())
            .and_then(|v| v.as_str().map(str::to_owned));
        let row = BookWork {
            work,
            anilist_id: Some(book.id.get()),
            medium: book.medium().as_str().to_owned(),
            format: Some(book.reading_format().as_str().to_owned()),
            origin: book.origin().map(|o| o.code().to_owned()),
            titles: titles.to_string(),
            cover: book.cover_image.best().map(str::to_owned),
            status,
            chapters_total: book.chapters,
            volumes_total: book.volumes,
            // As the catalogue writes it: the book, and no page asked yet.
            meta: Some(serde_json::json!({ "book": book, "detail_at": null }).to_string()),
            fetched_at: now,
        };
        if let Err(e) = store.put_book_work(&row) {
            tracing::warn!(error = %e, "a reading scene's book was not kept");
        }
    }
}
