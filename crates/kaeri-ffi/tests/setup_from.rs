//! "Already use Kaeri on another device?": a new device paired with one already set up gets its
//! book servers with their sign-ins and its answer to Watch, read, or both, and adds the servers
//! picked. A made-up Kavita answers on loopback.
#![cfg(feature = "testing")]

use std::{
    collections::HashMap,
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

use kaeri_ffi::{FfiDirs, FfiEngine, FfiHost, FfiStart};
use kaeri_model::{
    book_sources::BookSourceCommand,
    came_in::CameIn,
    command::Command,
    event::Event,
    group::{FirstRunPage, SyncCommand, SyncStatus},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

const PATIENCE: Duration = Duration::from_secs(30);
const KEY: &str = "0f1e2d3c-4b5a-6978-8a9b-acbdcedf1234";

struct Quiet;

impl FfiHost for Quiet {
    fn open_url(&self, _url: String) -> bool {
        false
    }
    fn notify(&self, _title: String, _body: String) {}
    fn secret_get(&self, _id: String) -> Option<String> {
        None
    }
    fn secret_set(&self, _id: String, _value: String) -> bool {
        false
    }
    fn secret_delete(&self, _id: String) -> bool {
        true
    }
}

struct App {
    engine: Arc<FfiEngine>,
    events: mpsc::Receiver<Event>,
    _root: tempfile::TempDir,
}

impl App {
    fn start() -> Self {
        let root = tempfile::tempdir().expect("a temporary directory");
        let dir = |name: &str| root.path().join(name).display().to_string();
        let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
        let start = FfiStart { reading: true, ..FfiStart::default() };
        let engine =
            FfiEngine::start_testing(dirs, Arc::new(Quiet), start).expect("the engine starts");
        let (tx, events) = mpsc::channel();
        let reader = Arc::clone(&engine);
        thread::spawn(move || {
            loop {
                let batch = futures::executor::block_on(reader.next_events());
                if batch.is_empty() {
                    return;
                }
                for event in batch {
                    if tx.send(event).is_err() {
                        return;
                    }
                }
            }
        });
        Self { engine, events, _root: root }
    }

    fn until<T>(&mut self, what: &str, mut want: impl FnMut(&Event) -> Option<T>) -> T {
        let deadline = Instant::now() + PATIENCE;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let Ok(event) = self.events.recv_timeout(left) else { panic!("no {what} in time") };
            if let Some(found) = want(&event) {
                return found;
            }
        }
    }

    fn came_in(&mut self, what: &str) -> CameIn {
        self.until(what, |event| match event {
            Event::CameIn(came) => Some(came.clone()),
            _ => None,
        })
    }

    fn status_where(&mut self, what: &str, ok: impl Fn(&SyncStatus) -> bool) -> SyncStatus {
        self.until(what, |event| match event {
            Event::SyncStatus(status) if ok(status) => Some(status.clone()),
            _ => None,
        })
    }
}

fn pair(asker: &mut App, asker_name: &str, holder: &mut App, holder_name: &str) {
    asker.engine.send(Command::PairInvite { device_name: asker_name.into() });
    let (code, port) = asker.until("the code", |event| match event {
        Event::SyncInvite { code, port, .. } => Some((code.clone(), *port)),
        _ => None,
    });
    holder.engine.send(Command::PairJoin {
        device_name: holder_name.into(),
        address: format!("127.0.0.1:{port}"),
        code,
    });
    holder.status_where("both devices", |s| s.devices.len() == 2);
}

/// A made-up Kavita on loopback, answering by path prefix, on a runtime of its own.
fn kavita() -> (String, tokio::runtime::Runtime) {
    let routes: HashMap<&'static str, (&'static str, &'static str)> = HashMap::from([
        (
            "/api/Plugin/authenticate",
            (
                "",
                r#"{"username":"kim","token":"jwt.key","refreshToken":"refresh.key","apiKey":"0f1e2d3c-4b5a-6978-8a9b-acbdcedf1234"}"#,
            ),
        ),
        ("/api/Server/server-info-slim", ("", r#"{"kavitaVersion":"0.8.4.0"}"#)),
        (
            "/api/Series/all-v2",
            (
                "Pagination: {\"currentPage\":1,\"itemsPerPage\":1,\"totalItems\":212,\"totalPages\":212}\r\n",
                "[]",
            ),
        ),
        (
            "/api/Library/libraries",
            ("", r#"[{"id":1,"name":"Manga","type":0},{"id":2,"name":"Comics","type":1}]"#),
        ),
        (
            "/api/Search/search",
            ("", r#"{"series":[{"seriesId":8,"name":"Paper Harbour","libraryId":2}]}"#),
        ),
    ]);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let address = runtime.block_on(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else { return };
                let routes = routes.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 16384];
                    let n = stream.read(&mut buf).await.unwrap_or(0);
                    let text = String::from_utf8_lossy(&buf[..n]).to_string();
                    let path = text.lines().next().unwrap_or_default().split(' ').nth(1);
                    let found = routes
                        .iter()
                        .filter(|(prefix, _)| path.is_some_and(|p| p.starts_with(**prefix)))
                        .max_by_key(|(prefix, _)| prefix.len())
                        .map(|(_, answer)| *answer);
                    let response = match found {
                        Some((headers, body)) => format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        ),
                        None => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
                    };
                    let _ = stream.write_all(response.as_bytes()).await;
                });
            }
        });
        address
    });
    (address, runtime)
}

#[test]
fn a_new_device_paired_with_one_set_up_gets_its_book_server_and_its_answer() {
    let (address, _server) = kavita();
    let mut phone = App::start();

    // The phone adds the server from a pasted Kavita OPDS link, with one of its two libraries.
    phone.engine.send(Command::BookSources(BookSourceCommand::Probe {
        address: format!("{address}/api/opds/{KEY}"),
        user: None,
        password: None,
    }));
    let CameIn::Found(found) = phone.came_in("the server found") else { panic!("not found") };
    assert_eq!(found.key_last4.as_deref(), Some("1234"));
    assert_eq!(found.series, Some(212));
    assert_eq!(found.libraries.len(), 2);
    assert!(!format!("{found:?}").contains(KEY), "the key never leaves the engine");
    phone.engine.send(Command::BookSources(BookSourceCommand::AddServer {
        pending: found.pending,
        libraries: vec!["2".into()],
        report_progress: true,
    }));
    let CameIn::ServerAdded(added) = phone.came_in("the server added") else {
        panic!("not added")
    };
    assert_eq!(added.libraries, 1);

    // The tablet's first run asks the next device to pair for its setup.
    let mut tablet = App::start();
    tablet
        .engine
        .send(Command::SyncGroup(SyncCommand::AskSetupFrom { device: None, address: None }));
    pair(&mut tablet, "Kitchen iPad", &mut phone, "Kim's phone");
    let setup = tablet.until("the setup", |event| match event {
        Event::SetupFrom(setup) => Some(setup.clone()),
        _ => None,
    });
    assert_eq!(setup.device_name, "Kim's phone");
    assert!(setup.medium.is_some());
    assert_eq!(setup.folders, 0);
    assert!(setup.answered.contains(&FirstRunPage::Medium));
    assert!(setup.answered.contains(&FirstRunPage::Sources));
    let [server] = setup.servers.as_slice() else { panic!("{:?}", setup.servers) };
    assert_eq!(server.id, added.id);
    assert_eq!(server.key_last4.as_deref(), Some("1234"));
    assert_eq!(server.libraries, ["2"]);
    assert!(!server.added);
    assert!(!format!("{setup:?}").contains(KEY), "the key is not in the event");
    tablet.status_where("the server offered", |s| s.book_servers_to_add.len() == 1);

    tablet.engine.send(Command::SyncGroup(SyncCommand::AddBookServersFrom {
        device: setup.device.clone(),
        servers: Vec::new(),
    }));
    let CameIn::ServerAdded(on_tablet) = tablet.came_in("the server added on the tablet")
    else {
        panic!("not added")
    };
    assert_eq!(on_tablet.id, added.id);
    assert_eq!(on_tablet.libraries, 1);
    tablet.status_where("nothing left to add", |s| s.book_servers_to_add.is_empty());
}
