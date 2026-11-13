//! What came in after a book server or a folder was added, and why nothing did, through the
//! Sources screen's commands. A made-up Komga answers on loopback; the folder is made here.

use std::{
    sync::{
        Arc,
        atomic::{AtomicU16, AtomicU32, Ordering},
    },
    time::Duration,
};

use kaeri_core::config::Paths;
use kaeri_engine::{
    book_plugins::BookPlugins,
    host::{BookFile, FolderEntry, FolderTree},
    read_sources::BookSources,
};
use kaeri_model::{
    book_sources::{BookServerKind, BookSourceCommand, FolderRef},
    came_in::{CameIn, NothingCameIn},
    event::Event,
    stores::Secret,
};
use kaeri_store::Store;
use kaeri_track::TokenStore;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::mpsc,
};

fn sources(dir: &std::path::Path) -> BookSources {
    let paths = Paths {
        config_file: dir.join("config.toml"),
        data_dir: dir.join("data"),
        cache_dir: dir.join("cache"),
    };
    let tokens = TokenStore::new(dir.join("tokens"));
    BookSources::new(Store::open_in_memory().unwrap(), tokens, reqwest::Client::new(), &paths)
}

fn run(
    sources: &BookSources,
    command: BookSourceCommand,
    now: i64,
) -> mpsc::UnboundedReceiver<Event> {
    let (tx, rx) = mpsc::unbounded_channel();
    kaeri_engine::read_sources::command(
        sources,
        &BookPlugins::default(),
        |_| None,
        command,
        now,
        &tx.into(),
    );
    rx
}

async fn came_in(rx: &mut mpsc::UnboundedReceiver<Event>) -> CameIn {
    loop {
        let event = tokio::time::timeout(Duration::from_secs(20), rx.recv())
            .await
            .expect("an event")
            .expect("open");
        if let Event::CameIn(came) = event {
            return came;
        }
    }
}

/// A made-up Komga: `series` is what it holds, and `me` the status its sign-in answers.
struct Komga {
    address: String,
    series: Arc<AtomicU32>,
    me: Arc<AtomicU16>,
}

async fn komga() -> Komga {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let series = Arc::new(AtomicU32::new(0));
    let me = Arc::new(AtomicU16::new(200));
    let (count, status) = (series.clone(), me.clone());
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else { return };
            let (count, status) = (count.clone(), status.clone());
            tokio::spawn(async move {
                let mut buf = vec![0u8; 16384];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                let text = String::from_utf8_lossy(&buf[..n]).to_string();
                let path =
                    text.lines().next().unwrap_or_default().split(' ').nth(1).unwrap_or("");
                let total = count.load(Ordering::SeqCst);
                let (code, body) = if path.starts_with("/api/v1/claim") {
                    (200, r#"{"isClaimed":true}"#.to_owned())
                } else if path.starts_with("/api/v1/users/me") {
                    let code = status.load(Ordering::SeqCst);
                    (
                        code,
                        r#"{"id":"u1","email":"kim@home.example","roles":["USER"]}"#.to_owned(),
                    )
                } else if path.starts_with("/actuator/info") {
                    (200, r#"{"build":{"version":"1.11.0"}}"#.to_owned())
                } else if path.starts_with("/api/v1/series") {
                    (200, format!(r#"{{"content":[],"totalElements":{total}}}"#))
                } else if path.starts_with("/api/v1/libraries") {
                    (200, r#"[{"id":"0A","name":"Manga"}]"#.to_owned())
                } else {
                    (404, String::new())
                };
                let response = format!(
                    "HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    });
    Komga { address, series, me }
}

fn probe(address: &str) -> BookSourceCommand {
    BookSourceCommand::Probe {
        address: address.to_owned(),
        user: Some("kim".into()),
        password: Some(Secret("a password".into())),
    }
}

#[tokio::test]
async fn an_empty_server_can_be_added_and_is_looked_at_again_half_an_hour_later() {
    let dir = tempfile::tempdir().unwrap();
    let sources = sources(dir.path());
    let server = komga().await;

    let came = came_in(&mut run(&sources, probe(&server.address), 1_000)).await;
    let CameIn::Empty(NothingCameIn::EmptyCatalogue { found }) = came else {
        panic!("{came:?}")
    };
    assert_eq!((found.kind, found.series), (BookServerKind::Komga, Some(0)));
    assert_eq!(found.libraries.len(), 1);

    let add = BookSourceCommand::AddServer {
        pending: found.pending,
        libraries: Vec::new(),
        report_progress: true,
    };
    let CameIn::ServerAdded(added) = came_in(&mut run(&sources, add, 1_000)).await else {
        panic!("not added")
    };
    assert!(added.empty);
    let row = sources
        .view(&BookPlugins::default(), |_| None)
        .servers
        .into_iter()
        .find(|s| s.id == added.id)
        .expect("its row");
    assert_eq!(row.rechecks_at, Some(1_000 + 30 * 60));

    // Not before its half hour.
    server.series.store(168, Ordering::SeqCst);
    assert!(sources.recheck(1_000 + 60, false).await.is_empty());
    let filled = sources.recheck(1_000 + 30 * 60, false).await;
    let [filled] = filled.as_slice() else { panic!("{filled:?}") };
    assert_eq!((filled.series, filled.empty), (Some(168), false));
    let row = sources
        .view(&BookPlugins::default(), |_| None)
        .servers
        .into_iter()
        .find(|s| s.id == added.id)
        .unwrap();
    assert_eq!(row.rechecks_at, None);
}

#[tokio::test]
async fn a_refused_password_and_an_address_nobody_answers_say_why() {
    let dir = tempfile::tempdir().unwrap();
    let sources = sources(dir.path());
    let server = komga().await;
    server.me.store(401, Ordering::SeqCst);
    let came = came_in(&mut run(&sources, probe(&server.address), 1_000)).await;
    assert!(
        matches!(
            &came,
            CameIn::Empty(NothingCameIn::WrongPassword { kind: BookServerKind::Komga, .. })
        ),
        "{came:?}"
    );

    let nobody = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://127.0.0.1:{}", nobody.local_addr().unwrap().port());
    drop(nobody);
    let came = came_in(&mut run(&sources, probe(&address), 1_000)).await;
    assert!(matches!(&came, CameIn::Empty(NothingCameIn::Unreachable { .. })), "{came:?}");
}

#[tokio::test]
async fn a_folder_of_books_kaeri_cannot_open_says_so_by_format() {
    let dir = tempfile::tempdir().unwrap();
    let sources = sources(dir.path());
    let books = dir.path().join("Shelf");
    std::fs::create_dir_all(&books).unwrap();
    for name in ["Lantern Keeper 01.mobi", "Lantern Keeper 02.mobi", "Paper Harbour.azw3"] {
        std::fs::write(books.join(name), b"not a book Kaeri reads").unwrap();
    }
    let add = BookSourceCommand::AddFolder {
        folder: FolderRef { path: Some(books.display().to_string()), ..FolderRef::default() },
    };
    let came = came_in(&mut run(&sources, add, 1_000)).await;
    let CameIn::Empty(NothingCameIn::UnsupportedFiles { folder }) = came else {
        panic!("{came:?}")
    };
    assert_eq!((folder.series, folder.files, folder.skipped()), (0, 3, 3));
    let formats: Vec<_> =
        folder.unsupported.iter().map(|f| (f.format.as_str(), f.count)).collect();
    assert_eq!(formats, [("MOBI", 2), ("AZW3", 1)]);
}

/// A folder the platform opens, as a picked Android folder: a folder on disk named by
/// references under a scheme of its own, which `refused` can close.
struct PickedFolder {
    root: std::path::PathBuf,
    refused: std::sync::atomic::AtomicBool,
}

const PICKED: &str = "picked-books://shelf";

impl PickedFolder {
    fn path_of(&self, reference: &str) -> std::io::Result<std::path::PathBuf> {
        if self.refused.load(Ordering::Relaxed) {
            return Err(std::io::ErrorKind::PermissionDenied.into());
        }
        let rest = reference.strip_prefix(PICKED).ok_or(std::io::ErrorKind::NotFound)?;
        Ok(self.root.join(rest.trim_start_matches('/')))
    }

    fn describe(&self, path: &std::path::Path) -> std::io::Result<FolderEntry> {
        let meta = std::fs::metadata(path)?;
        let rel = path.strip_prefix(&self.root).unwrap_or(path).to_string_lossy();
        Ok(FolderEntry {
            reference: format!("{PICKED}/{rel}"),
            name: path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
            folder: meta.is_dir(),
            size: meta.len(),
            modified: 1,
        })
    }
}

impl FolderTree for PickedFolder {
    fn claims(&self, reference: &str) -> bool {
        reference.starts_with(PICKED)
    }

    fn entries(&self, reference: &str) -> std::io::Result<Vec<FolderEntry>> {
        std::fs::read_dir(self.path_of(reference)?)?
            .map(|entry| self.describe(&entry?.path()))
            .collect()
    }

    fn entry(&self, reference: &str) -> std::io::Result<FolderEntry> {
        self.describe(&self.path_of(reference)?)
    }

    fn open(&self, reference: &str) -> std::io::Result<Box<dyn BookFile>> {
        Ok(Box::new(std::fs::File::open(self.path_of(reference)?)?))
    }
}

fn cbz(path: &std::path::Path, pages: usize) {
    use std::io::Write;
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut png = Vec::new();
    image::DynamicImage::new_rgb8(2, 3)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    for i in 1..=pages {
        zip.start_file(format!("{i:03}.png"), zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&png).unwrap();
    }
    zip.finish().unwrap();
}

fn picked() -> BookSourceCommand {
    BookSourceCommand::AddFolder {
        folder: FolderRef {
            tree_uri: Some(PICKED.into()),
            name: Some("Shelf".into()),
            ..FolderRef::default()
        },
    }
}

fn folder_scan(sources: &BookSources) -> kaeri_model::book_sources::BookFolderScan {
    let view = sources.view(&BookPlugins::default(), |_| None);
    view.folders.into_iter().next().expect("the folder").scan
}

#[tokio::test]
async fn a_folder_the_platform_opens_is_scanned_and_its_books_read_through_it() {
    let dir = tempfile::tempdir().unwrap();
    let sources = sources(dir.path());
    let shelf = dir.path().join("Shelf");
    cbz(&shelf.join("Paper Lantern/Paper Lantern ch001.cbz"), 3);
    cbz(&shelf.join("Paper Lantern/Paper Lantern ch002.cbz"), 2);
    let tree = Arc::new(PickedFolder { root: shelf, refused: false.into() });
    sources.open_folders_through(Some(tree.clone()));

    let came = came_in(&mut run(&sources, picked(), 1_000)).await;
    let CameIn::Folder(report) = came else { panic!("{came:?}") };
    assert_eq!((report.name.as_str(), report.series, report.books), ("Shelf", 1, 2));
    assert_eq!(folder_scan(&sources), kaeri_model::book_sources::BookFolderScan::Done);

    // The books are kept by the platform's references, and open through it.
    let path =
        std::path::PathBuf::from(format!("{PICKED}/Paper Lantern/Paper Lantern ch002.cbz"));
    let book = kaeri_books::open(&path, &kaeri_books::Limits::default()).unwrap();
    let kaeri_books::Book::Comic(comic) = book else { panic!("not pages") };
    assert_eq!(comic.pages().len(), 2);

    // Once the platform no longer lets Kaeri in, a scan says so and the folder shows as not
    // found, rather than waiting.
    tree.refused.store(true, Ordering::Relaxed);
    let came = came_in(&mut run(&sources, picked(), 2_000)).await;
    assert!(matches!(&came, CameIn::Empty(NothingCameIn::FolderUnreadable { .. })), "{came:?}");
    assert_eq!(folder_scan(&sources), kaeri_model::book_sources::BookFolderScan::Missing);
    sources.open_folders_through(None);
}

#[tokio::test]
async fn a_folder_nothing_can_open_says_so_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let sources = sources(dir.path());
    // A picked folder, and no way into it from this host.
    let add = BookSourceCommand::AddFolder {
        folder: FolderRef {
            tree_uri: Some("unopened-books://shelf".into()),
            name: Some("Shelf".into()),
            ..FolderRef::default()
        },
    };
    let came = came_in(&mut run(&sources, add, 1_000)).await;
    let CameIn::Empty(NothingCameIn::FolderUnreadable { folder }) = came else {
        panic!("{came:?}")
    };
    assert_eq!((folder.name.as_str(), folder.series, folder.files), ("Shelf", 0, 0));
    assert_eq!(folder_scan(&sources), kaeri_model::book_sources::BookFolderScan::Missing);

    // A path that is no folder says the same.
    let add = BookSourceCommand::AddFolder {
        folder: FolderRef {
            path: Some(dir.path().join("not-there").display().to_string()),
            ..FolderRef::default()
        },
    };
    let came = came_in(&mut run(&sources, add, 1_000)).await;
    assert!(matches!(&came, CameIn::Empty(NothingCameIn::FolderUnreadable { .. })), "{came:?}");

    // As does a folder with nothing to find it by.
    let add = BookSourceCommand::AddFolder { folder: FolderRef::default() };
    let came = came_in(&mut run(&sources, add, 1_000)).await;
    assert!(matches!(&came, CameIn::Empty(NothingCameIn::FolderUnreadable { .. })), "{came:?}");
}
