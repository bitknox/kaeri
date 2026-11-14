//! A big folder scans quickly the first time and again: 10 000 made-up books in 200 series,
//! added and then scanned once more, say their progress a few times a second rather than once
//! a book, and the second scan costs about what the first did. Removed while it is scanned, the
//! folder stops and says nothing, until an undo scans it again.

use std::time::{Duration, Instant};

use kaeri_core::config::Paths;
use kaeri_engine::{
    book_plugins::BookPlugins,
    read_sources::{BookSources, PROGRESS_EVERY},
};
use kaeri_model::{
    book_sources::{BookFolderScan, BookSourceCommand, BookSourceEvent, FolderRef},
    came_in::CameIn,
    event::Event,
};
use kaeri_store::Store;
use kaeri_track::TokenStore;
use tokio::sync::mpsc;

const SERIES: usize = 200;
const CHAPTERS: usize = 50;

/// One small book, copied into every chapter's place.
fn tree(root: &std::path::Path) {
    use std::io::Write;
    let mut png = Vec::new();
    image::DynamicImage::new_rgb8(2, 3)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let book = root.join("book.cbz");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&book).unwrap());
    zip.start_file("001.png", zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(&png).unwrap();
    zip.finish().unwrap();
    let shelf = root.join("shelf");
    for s in 0..SERIES {
        let name = format!("Made-up Series {s:03}");
        let dir = shelf.join(&name);
        std::fs::create_dir_all(&dir).unwrap();
        for c in 1..=CHAPTERS {
            std::fs::copy(&book, dir.join(format!("{name} Ch. {c:03}.cbz"))).unwrap();
        }
    }
}

/// Made-up books, the store and the sources over it, and the command that adds the books.
fn shelf() -> (tempfile::TempDir, Store, BookSources, BookSourceCommand) {
    let dir = tempfile::tempdir().unwrap();
    tree(dir.path());
    let paths = Paths {
        config_file: dir.path().join("config.toml"),
        data_dir: dir.path().join("data"),
        cache_dir: dir.path().join("cache"),
    };
    let store = Store::open_in_memory().unwrap();
    let sources = BookSources::new(
        store.clone(),
        TokenStore::new(dir.path().join("tokens")),
        reqwest::Client::new(),
        &paths,
    );
    let shelf = dir.path().join("shelf").to_string_lossy().into_owned();
    let add = BookSourceCommand::AddFolder {
        folder: FolderRef { path: Some(shelf), ..FolderRef::default() },
    };
    (dir, store, sources, add)
}

fn send(
    sources: &BookSources,
    command: BookSourceCommand,
    at: i64,
) -> mpsc::UnboundedReceiver<Event> {
    let (tx, rx) = mpsc::unbounded_channel();
    kaeri_engine::read_sources::command(
        sources,
        &BookPlugins::default(),
        |_| None,
        command,
        at,
        &tx.into(),
    );
    rx
}

async fn next(rx: &mut mpsc::UnboundedReceiver<Event>) -> Event {
    tokio::time::timeout(Duration::from_secs(120), rx.recv())
        .await
        .expect("an event within two minutes")
        .expect("open")
}

/// Run `command`, and take the views it says until the folder was scanned at `at` and a
/// view says it is done: how many views came, counting any just after, and how long it took.
async fn scanned(
    sources: &BookSources,
    store: &Store,
    command: BookSourceCommand,
    at: i64,
) -> (usize, Duration) {
    let started = Instant::now();
    let mut rx = send(sources, command, at);
    let mut views = 0;
    loop {
        let event = next(&mut rx).await;
        let Event::BookSources(BookSourceEvent::View(view)) = event else { continue };
        views += 1;
        let kept = store.book_folders().unwrap().first().and_then(|f| f.last_scan);
        if kept == Some(at)
            && view.folders.first().is_some_and(|f| f.scan == BookFolderScan::Done)
        {
            let took = started.elapsed();
            // Whatever comes after it counts too.
            while let Ok(Some(event)) =
                tokio::time::timeout(Duration::from_millis(600), rx.recv()).await
            {
                views +=
                    usize::from(matches!(event, Event::BookSources(BookSourceEvent::View(_))));
            }
            return (views, took);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_big_folder_is_scanned_again_as_quickly_with_a_few_updates_a_second() {
    let (_dir, store, sources, add) = shelf();

    let (first_views, first) = scanned(&sources, &store, add, 1).await;
    let view = sources.view(&BookPlugins::default(), |_| None);
    assert_eq!(view.groups.len(), SERIES);
    assert!(view.groups.iter().all(|g| g.files == CHAPTERS as u32));

    let (again_views, again) = scanned(&sources, &store, BookSourceCommand::Scan, 2).await;

    // A view a tick at most, and a few more for the folder added, the scan done and the
    // servers looked at.
    let most = |took: Duration| (took.as_millis() / PROGRESS_EVERY.as_millis()) as usize + 4;
    assert!(first_views <= most(first), "{first_views} views in {first:?}");
    assert!(again_views <= most(again), "{again_views} views in {again:?}");
    // Loose enough for a slow runner: the second scan took minutes when every book rebuilt
    // the whole view from the store.
    assert!(again < Duration::from_secs(60), "the second scan took {again:?}");
    assert!(again < first * 5 + Duration::from_secs(5), "{first:?}, then {again:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_folder_removed_while_it_is_scanned_stops_and_says_nothing_until_it_is_brought_back()
{
    let (_dir, store, sources, add) = shelf();
    let mut adding = send(&sources, add, 1);
    let id = loop {
        match next(&mut adding).await {
            Event::BookSources(BookSourceEvent::View(view)) => {
                if let Some(folder) = view.folders.first()
                    && matches!(folder.scan, BookFolderScan::Scanning { found } if found > 0)
                {
                    break folder.id.clone();
                }
            }
            Event::CameIn(came) => {
                panic!("the scan ended before it could be removed: {came:?}")
            }
            _ => {}
        }
    };

    let removed = Instant::now();
    let _ = send(&sources, BookSourceCommand::RemoveFolder { id: id.clone() }, 1);
    loop {
        match next(&mut adding).await {
            Event::BookSources(BookSourceEvent::View(view))
                if view.folders.first().is_some_and(|f| f.scan == BookFolderScan::Done) =>
            {
                break;
            }
            Event::CameIn(came) => panic!("a removed folder came in: {came:?}"),
            _ => {}
        }
    }
    assert!(removed.elapsed() < Duration::from_secs(5), "stopped in {:?}", removed.elapsed());
    while let Ok(Some(event)) =
        tokio::time::timeout(Duration::from_millis(800), adding.recv()).await
    {
        assert!(!matches!(event, Event::CameIn(_)), "a removed folder came in: {event:?}");
    }
    assert!(store.local_book_series().unwrap().is_empty(), "nothing of it is kept");

    // Brought back within the undo: scanned again, and said as the add would have said it.
    let mut back = send(&sources, BookSourceCommand::RestoreFolder { id }, 1);
    let came = loop {
        if let Event::CameIn(came) = next(&mut back).await {
            break came;
        }
    };
    let CameIn::Folder(report) = came else { panic!("{came:?}") };
    assert_eq!((report.series, report.books), (SERIES as u32, (SERIES * CHAPTERS) as u32));
    assert!(!report.already_added);
    assert_eq!(store.local_book_series().unwrap().len(), SERIES);
}
