//! The queue against fake Komga and Kavita servers on the loopback: offline, no answer, a refused
//! key, the newest mark, and a restart.

use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, Ordering},
    },
    time::Duration,
};

use kaeri_core::{books::BookHints, config::Paths};
use kaeri_model::{book_sources::BookServerRow, event::Event};
use kaeri_store::{Store, book_sources::BookServer};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::mpsc,
};

use super::*;
use crate::book_plugins::BookPlugins;

const WORK: &str = "a:900777";
const UP: u8 = 0;
/// Takes the connection and closes it without a word.
const SILENT: u8 = 1;
/// Answers 401.
const REFUSING: u8 = 2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Flavour {
    Komga,
    Kavita,
}

impl Flavour {
    fn kind(self) -> &'static str {
        match self {
            Self::Komga => "komga",
            Self::Kavita => "kavita",
        }
    }

    fn series(self) -> &'static str {
        match self {
            Self::Komga => "s1",
            Self::Kavita => "7",
        }
    }

    fn credential(self) -> &'static str {
        match self {
            Self::Komga => "token:tok-one",
            Self::Kavita => {
                r#"kavita:{"token":"jwt.one","refreshToken":"refresh.one","apiKey":"key-one"}"#
            }
        }
    }

    fn routes(self) -> Vec<(&'static str, &'static str)> {
        match self {
            Self::Komga => vec![
                (
                    "/api/v1/series/s1/books",
                    r#"{"content":[{"id":"b1","name":"Lantern Keeper 001","sizeBytes":1000,"media":{"status":"READY","mediaType":"application/zip","pagesCount":3},"metadata":{"title":"","number":"1"}},{"id":"b2","name":"Lantern Keeper 002","sizeBytes":1000,"media":{"status":"READY","mediaType":"application/zip","pagesCount":2},"metadata":{"title":"","number":"2"}}],"totalElements":2}"#,
                ),
                ("/api/v1/books/", "{}"),
            ],
            Self::Kavita => vec![
                (
                    "/api/Series/series-detail",
                    r#"{"specials":[],"chapters":[],"volumes":[{"id":11,"name":"1","chapters":[{"id":901,"range":"1","titleName":"","title":"","pages":3,"isSpecial":false,"files":[{"format":1}]},{"id":902,"range":"2","titleName":"","title":"","pages":2,"isSpecial":false,"files":[{"format":1}]}]}],"storylineChapters":[]}"#,
                ),
                ("/api/Reader/mark-multiple", "{}"),
            ],
        }
    }

    /// Whether a request wrote progress.
    fn writes(self, method: &str, path: &str) -> bool {
        match self {
            Self::Komga => path.ends_with("/read-progress") && method != "GET",
            Self::Kavita => path.starts_with("/api/Reader/mark-multiple"),
        }
    }
}

type Seen = Arc<Mutex<Vec<(String, String)>>>;

/// A fake server: canned answers by path prefix while up, every request kept.
async fn serve(flavour: Flavour, mode: Arc<AtomicU8>) -> (String, Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let seen: Seen = Arc::default();
    let log = seen.clone();
    let routes = flavour.routes();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else { return };
            let (routes, log, mode) = (routes.clone(), log.clone(), mode.clone());
            tokio::spawn(async move {
                let mut buf = vec![0u8; 16384];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                let text = String::from_utf8_lossy(&buf[..n]).to_string();
                let mut first = text.lines().next().unwrap_or_default().split(' ');
                let (method, path) = (first.next().unwrap_or(""), first.next().unwrap_or(""));
                log.lock().unwrap().push((method.to_owned(), path.to_owned()));
                let response = match mode.load(Ordering::SeqCst) {
                    SILENT => return,
                    REFUSING => {
                        "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            .to_owned()
                    }
                    _ => match routes
                        .iter()
                        .filter(|(prefix, _)| path.starts_with(prefix))
                        .max_by_key(|(prefix, _)| prefix.len())
                    {
                        Some((_, json)) => format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
                            json.len()
                        ),
                        None => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned(),
                    },
                };
                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    });
    (address, seen)
}

struct Kit {
    flavour: Flavour,
    sources: BookSources,
    mode: Arc<AtomicU8>,
    seen: Seen,
    address: String,
    /// The chapter keys, in the server's order.
    keys: Vec<ChapterId>,
    dir: Arc<tempfile::TempDir>,
}

fn sources_at(dir: &Path, store: Store) -> BookSources {
    let paths = Paths {
        config_file: dir.join("config.toml"),
        data_dir: dir.join("data"),
        cache_dir: dir.join("cache"),
    };
    let tokens = kaeri_track::TokenStore::with_backend(
        dir.join("tokens"),
        kaeri_track::secret::Backend::File,
    );
    BookSources::new(store, tokens, reqwest::Client::new(), &paths)
}

async fn kit(flavour: Flavour) -> Kit {
    let dir = Arc::new(tempfile::tempdir().unwrap());
    let mode = Arc::new(AtomicU8::new(UP));
    let (address, seen) = serve(flavour, mode.clone()).await;
    let store = Store::open(dir.path().join("kaeri.db")).unwrap();
    store
        .put_book_server(&BookServer {
            id: "k".into(),
            kind: flavour.kind().into(),
            name: "Home shelf".into(),
            address: address.clone(),
            user: Some("reader".into()),
            report_progress: true,
            signed_out: false,
            last_seen: None,
        })
        .unwrap();
    store.set_book_match("k", flavour.series(), WORK, 90, true, false, 1).unwrap();
    let tokens = kaeri_track::TokenStore::with_backend(
        dir.path().join("tokens"),
        kaeri_track::secret::Backend::File,
    );
    tokens.set("book-server/k", flavour.credential()).unwrap();
    let sources = sources_at(dir.path(), store);
    let listed = sources
        .registry
        .chapters("k", flavour.series(), &BookHints::default(), 1)
        .await
        .unwrap();
    let restarts = crate::read_sources::restarts_per_volume(&listed);
    let keys = listed.iter().map(|c| c.key(restarts).id()).collect();
    seen.lock().unwrap().clear();
    Kit { flavour, sources, mode, seen, address, keys, dir }
}

impl Kit {
    fn set(&self, mode: u8) {
        self.mode.store(mode, Ordering::SeqCst);
    }

    /// The progress writes the server got, as method and path.
    fn writes(&self) -> Vec<(String, String)> {
        let seen = self.seen.lock().unwrap();
        seen.iter().filter(|(m, p)| self.flavour.writes(m, p)).cloned().collect()
    }

    fn asked(&self) -> usize {
        self.seen.lock().unwrap().len()
    }

    fn store(&self) -> &Store {
        self.sources.store()
    }

    fn row(&self) -> BookServerRow {
        let view = self.sources.view(&BookPlugins::default(), |_| None);
        view.servers.into_iter().find(|s| s.id == "k").unwrap()
    }

    fn work(&self) -> WorkId {
        WorkId::new(WORK)
    }
}

fn results(rx: &mut mpsc::UnboundedReceiver<Event>) -> Vec<(String, WriteBackResult)> {
    let mut out = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let Event::Reader(ReaderEvent::ProgressSent { chapter, result, .. }) = event {
            out.push((chapter.as_str().to_owned(), result));
        }
    }
    out
}

#[tokio::test]
async fn progress_made_offline_goes_once_back_online_and_never_again() {
    for flavour in [Flavour::Komga, Flavour::Kavita] {
        let kit = kit(flavour).await;
        kit.sources.write_back_network(true, 100);
        kit.sources.report(&kit.work(), vec![kit.keys[0].clone()], true, 100);
        assert_eq!(kit.sources.send_progress_due(100).await, Pass::default());
        assert!(kit.writes().is_empty(), "{flavour:?}: nothing goes offline");
        let row = kit.row();
        assert_eq!(row.progress_pending, 1);
        assert_eq!(row.progress_problem, Some(WriteBackProblem::Offline));

        kit.sources.write_back_network(false, 200);
        let pass = kit.sources.send_progress_due(200).await;
        assert_eq!(pass.sent, 1, "{flavour:?}");
        assert_eq!(kit.writes().len(), 1, "{flavour:?}: {:?}", kit.writes());
        let row = kit.row();
        assert_eq!(
            (row.progress_pending, row.progress_saved_at, row.progress_problem),
            (0, Some(200), None)
        );

        // Sent is gone: later passes send nothing more.
        assert_eq!(kit.sources.send_progress_due(100_000).await, Pass::default());
        assert_eq!(kit.writes().len(), 1, "{flavour:?}");
    }
}

#[tokio::test]
async fn a_server_that_does_not_answer_is_tried_on_the_backoff_and_then_waits_for_try_again() {
    let kit = kit(Flavour::Komga).await;
    kit.set(SILENT);
    let t0 = 1_000;
    kit.sources.report(&kit.work(), vec![kit.keys[0].clone(), kit.keys[1].clone()], true, t0);
    let pass = kit.sources.send_progress_due(t0).await;
    assert_eq!((pass.sent, pass.later, pass.held), (0, 2, 0));
    // One ask found the server silent; the other chapter waited with it.
    assert_eq!(kit.asked(), 1);
    let row = kit.row();
    assert_eq!(row.progress_pending, 2);
    assert_eq!(row.progress_problem, Some(WriteBackProblem::NoAnswer));
    assert_eq!(row.progress_problem_at, Some(t0));

    // Nothing is asked before its time; then 30 s, 2 min, 10 min, and hourly.
    assert_eq!(kit.sources.send_progress_due(t0 + 29).await, Pass::default());
    let mut at = t0;
    let mut waits = Vec::new();
    loop {
        let next = kit.store().next_book_progress_due().unwrap();
        let Some(next) = next else { break };
        waits.push(next - at);
        at = next;
        kit.sources.send_progress_due(at).await;
        assert!(waits.len() < 40, "it never gave up: {waits:?}");
    }
    assert_eq!(&waits[..4], [30, 120, 600, 3_600]);
    assert!(waits[4..].iter().all(|w| *w == 3_600), "{waits:?}");
    assert!(at - t0 >= 86_400, "a whole day was tried: {}", at - t0);
    let row = kit.row();
    assert_eq!(row.progress_pending, 2, "nothing leaves the queue on its own");
    assert_eq!(row.progress_problem, Some(WriteBackProblem::NoAnswerForADay));
    let asked = kit.asked();
    assert_eq!(kit.sources.send_progress_due(at + 1_000_000).await, Pass::default());
    assert_eq!(kit.asked(), asked, "held marks wait for Try again");

    // Try again with the server back: both go, once.
    kit.set(UP);
    kit.sources.retry_progress("k", at + 10);
    let pass = kit.sources.send_progress_due(at + 10).await;
    assert_eq!(pass.sent, 2);
    assert_eq!(kit.writes().len(), 2);
    assert_eq!(kit.row().progress_pending, 0);
    assert_eq!(kit.row().progress_problem, None);
}

#[tokio::test]
async fn a_refused_key_waits_for_try_again_and_is_not_retried_alone() {
    for flavour in [Flavour::Komga, Flavour::Kavita] {
        let kit = kit(flavour).await;
        kit.set(REFUSING);
        kit.sources.report(&kit.work(), vec![kit.keys[0].clone()], true, 100);
        let pass = kit.sources.send_progress_due(100).await;
        assert_eq!(pass.held, 1, "{flavour:?}");
        assert_eq!(kit.row().progress_problem, Some(WriteBackProblem::RefusedKey));
        let asked = kit.asked();
        assert_eq!(kit.sources.send_progress_due(1_000_000).await, Pass::default());
        assert_eq!(kit.asked(), asked, "{flavour:?}: a refusal is not retried on its own");

        // The key was fixed on the server: Try again asks once more and it goes.
        kit.set(UP);
        kit.sources.retry_progress("k", 2_000_000);
        assert_eq!(kit.sources.send_progress_due(2_000_000).await.sent, 1, "{flavour:?}");
        assert_eq!(kit.writes().len(), 1);
        assert_eq!(kit.row().progress_saved_at, Some(2_000_000));
    }
}

#[tokio::test]
async fn the_newest_mark_for_a_chapter_is_the_one_sent() {
    let kit = kit(Flavour::Komga).await;
    kit.sources.write_back_network(true, 100);
    kit.sources.report(&kit.work(), vec![kit.keys[0].clone()], true, 100);
    kit.sources.report(&kit.work(), vec![kit.keys[0].clone()], false, 110);
    kit.sources.write_back_network(false, 120);
    assert_eq!(kit.sources.send_progress_due(120).await.sent, 1);
    let writes = kit.writes();
    assert_eq!(writes.len(), 1, "{writes:?}");
    assert_eq!(writes[0], ("DELETE".to_owned(), "/api/v1/books/b1/read-progress".to_owned()));
}

#[tokio::test]
async fn a_mark_made_offline_waits_across_a_restart_and_goes_at_launch() {
    let kit = kit(Flavour::Komga).await;
    let (tx, mut rx) = mpsc::unbounded_channel();
    let tx = Outbox::from(tx);
    kit.sources.write_back_network(true, 100);
    kit.sources.report_read(&kit.work(), &kit.keys[1], kaeri_store::now(), &tx);
    assert_eq!(
        results(&mut rx),
        [(
            kit.keys[1].as_str().to_owned(),
            WriteBackResult::Waiting { problem: WriteBackProblem::Offline }
        )]
    );
    let (dir, address, seen, flavour) =
        (kit.dir.clone(), kit.address.clone(), kit.seen.clone(), kit.flavour);
    drop(kit);

    // The app starts again, online.
    let store = Store::open(dir.path().join("kaeri.db")).unwrap();
    let sources = sources_at(dir.path(), store);
    assert_eq!(sources.store().book_progress_state("k").unwrap().pending, 1);
    sources.start_write_back(crate::clock::Clock::default());
    for _ in 0..500 {
        if sources.store().book_progress_state("k").unwrap().pending == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let writes: Vec<_> =
        seen.lock().unwrap().iter().filter(|(m, p)| flavour.writes(m, p)).cloned().collect();
    assert_eq!(writes, [("PATCH".to_owned(), "/api/v1/books/b2/read-progress".to_owned())]);
    let state = sources.store().book_progress_state("k").unwrap();
    assert_eq!((state.pending, state.problem), (0, None));
    assert!(state.sent_at.is_some());
    assert!(address.starts_with("http://127.0.0.1"));
}

#[tokio::test]
async fn the_reader_hears_what_became_of_the_chapter_it_read() {
    let kit = kit(Flavour::Kavita).await;
    kit.sources.start_write_back(crate::clock::Clock::default());
    let (tx, mut rx) = mpsc::unbounded_channel();
    let tx = Outbox::from(tx);
    kit.sources.report_read(&kit.work(), &kit.keys[0], kaeri_store::now(), &tx);
    let event = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.unwrap().unwrap();
    match event {
        Event::Reader(ReaderEvent::ProgressSent { chapter, server, host, result, .. }) => {
            assert_eq!(chapter, kit.keys[0]);
            assert_eq!((server.as_str(), host.as_str()), ("k", "127.0.0.1"));
            assert_eq!(result, WriteBackResult::Saved);
        }
        other => panic!("{other:?}"),
    }

    // A server set not to receive says so at once, and keeps no queue.
    kit.store()
        .put_book_server(&BookServer {
            report_progress: false,
            ..kit.store().book_server("k").unwrap().unwrap()
        })
        .unwrap();
    kit.sources.report_read(&kit.work(), &kit.keys[1], kaeri_store::now(), &tx);
    assert_eq!(
        results(&mut rx),
        [(
            kit.keys[1].as_str().to_owned(),
            WriteBackResult::NotSaved { problem: WriteBackProblem::NotSetToReceive }
        )]
    );
    let row = kit.row();
    assert_eq!(row.progress_pending, 0);
    assert_eq!(row.progress_problem, Some(WriteBackProblem::NotSetToReceive));
}

#[tokio::test]
async fn the_reader_knows_the_host_progress_goes_to_before_a_chapter_is_read() {
    let kit = kit(Flavour::Kavita).await;
    assert_eq!(kit.sources.progress_host(&kit.work()).as_deref(), Some("127.0.0.1"));
    // A work from no server, and a server set not to receive, name no host.
    assert_eq!(kit.sources.progress_host(&WorkId::new("work-from-nowhere")), None);
    kit.store()
        .put_book_server(&BookServer {
            report_progress: false,
            ..kit.store().book_server("k").unwrap().unwrap()
        })
        .unwrap();
    assert_eq!(kit.sources.progress_host(&kit.work()), None);
}

#[test]
fn the_waits_are_30_s_2_min_10_min_then_hourly() {
    let waits: Vec<i64> = (1..=6).map(retry_in).collect();
    assert_eq!(waits, [30, 120, 600, 3_600, 3_600, 3_600]);
}
