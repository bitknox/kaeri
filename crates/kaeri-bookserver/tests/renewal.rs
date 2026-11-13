//! A sign-in that lapses: a Kavita JWT the server stops taking is renewed with the refresh
//! token, or failing that the API key, once for every call refused at the same time, and the
//! new tokens go to the keeper. A renewal turned down, or a Komga session that lapsed, signs
//! the source out once. The fake server below keeps which JWT it takes.

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use kaeri_bookserver::{
    Gate, Keeper, Kind, Server, count_series_kept, kavita::Tokens, source, source_with,
};
use kaeri_core::{
    books::{BookHints, ChapterContent},
    error::ProviderError,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

const DETAIL: &str = r#"{"specials":[],"chapters":[],"volumes":[{"id":11,"name":"1","chapters":[{"id":901,"range":"1","titleName":"","title":"","pages":3,"isSpecial":false,"files":[{"format":1}]},{"id":902,"range":"2","titleName":"","title":"","pages":2,"isSpecial":false,"files":[{"format":1}]}]}],"storylineChapters":[]}"#;

/// How the fake answers a renewal.
#[derive(Clone)]
struct Plan {
    /// What `api/Account/refresh-token` answers: new JWT and refresh token, or `None` for 401.
    refresh: Option<(&'static str, &'static str)>,
    /// A status the refresh answers instead, as a server that cannot answer now.
    refresh_status: Option<u16>,
    /// How long the refresh takes.
    refresh_wait: Duration,
    /// What the key sign-in answers: a new JWT, or `None` for 401.
    key: Option<&'static str>,
}

impl Plan {
    fn rotating() -> Self {
        Self {
            refresh: Some(("jwt.two", "refresh.two")),
            refresh_status: None,
            refresh_wait: Duration::ZERO,
            key: None,
        }
    }
}

/// One request: method, path, the Authorization header and the body.
type Request = (String, String, String, String);

struct Fake {
    address: String,
    seen: Arc<Mutex<Vec<Request>>>,
}

impl Fake {
    fn count(&self, path: &str) -> usize {
        self.seen.lock().unwrap().iter().filter(|r| r.1.starts_with(path)).count()
    }

    fn requests(&self, path: &str) -> Vec<Request> {
        self.seen.lock().unwrap().iter().filter(|r| r.1.starts_with(path)).cloned().collect()
    }
}

async fn read_request(stream: &mut tokio::net::TcpStream) -> Option<String> {
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = stream.read(&mut buf).await.ok()?;
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
        let text = String::from_utf8_lossy(&data);
        if let Some(end) = text.find("\r\n\r\n") {
            let length = text[..end]
                .lines()
                .find_map(|l| {
                    let (name, value) = l.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse().ok())?
                })
                .unwrap_or(0usize);
            if data.len() >= end + 4 + length {
                break;
            }
        }
    }
    Some(String::from_utf8_lossy(&data).into_owned())
}

/// A Kavita that takes `jwt.one` until it is renewed as `plan` says, or a Komga that takes no
/// session at all.
async fn serve(plan: Plan) -> Fake {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let seen: Arc<Mutex<Vec<Request>>> = Arc::default();
    let accepted = Arc::new(Mutex::new("jwt.one".to_owned()));
    let log = seen.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else { return };
            let (log, accepted, plan) = (log.clone(), accepted.clone(), plan.clone());
            tokio::spawn(async move {
                let Some(text) = read_request(&mut stream).await else { return };
                let mut first = text.lines().next().unwrap_or_default().split(' ');
                let (method, path) = (
                    first.next().unwrap_or("").to_owned(),
                    first.next().unwrap_or("").to_owned(),
                );
                let auth = text
                    .lines()
                    .find_map(|l| {
                        let (name, value) = l.split_once(':')?;
                        name.eq_ignore_ascii_case("authorization")
                            .then(|| value.trim().to_owned())
                    })
                    .unwrap_or_default();
                let body =
                    text.split_once("\r\n\r\n").map(|(_, b)| b.to_owned()).unwrap_or_default();
                log.lock().unwrap().push((method, path.clone(), auth.clone(), body));
                let (status, headers, answer) = if path
                    .starts_with("/api/Account/refresh-token")
                {
                    tokio::time::sleep(plan.refresh_wait).await;
                    match (plan.refresh_status, plan.refresh) {
                        (Some(status), _) => (status, String::new(), String::new()),
                        (None, Some((jwt, refresh))) => {
                            *accepted.lock().unwrap() = jwt.to_owned();
                            (
                                200,
                                String::new(),
                                format!(r#"{{"token":"{jwt}","refreshToken":"{refresh}"}}"#),
                            )
                        }
                        (None, None) => (401, String::new(), String::new()),
                    }
                } else if path.starts_with("/api/Plugin/authenticate") {
                    match plan.key {
                        Some(jwt) if path.contains("apiKey=key-one") => {
                            *accepted.lock().unwrap() = jwt.to_owned();
                            (
                                200,
                                String::new(),
                                format!(
                                    r#"{{"token":"{jwt}","refreshToken":"refresh.key","apiKey":"key-one"}}"#
                                ),
                            )
                        }
                        _ => (401, String::new(), String::new()),
                    }
                } else if auth != format!("Bearer {}", accepted.lock().unwrap()) {
                    (401, String::new(), String::new())
                } else if path.starts_with("/api/Series/series-detail") {
                    (200, String::new(), DETAIL.to_owned())
                } else if path.starts_with("/api/Reader/chapter-info") {
                    (200, String::new(), r#"{"pages":3}"#.to_owned())
                } else if path.starts_with("/api/Reader/image") {
                    (200, String::new(), "image".to_owned())
                } else if path.starts_with("/api/Series/all-v2") {
                    (
                        200,
                        "Pagination: {\"currentPage\":1,\"itemsPerPage\":1,\"totalItems\":212,\"totalPages\":212}\r\n".to_owned(),
                        "[]".to_owned(),
                    )
                } else {
                    (404, String::new(), String::new())
                };
                let response = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{answer}",
                    answer.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    });
    Fake { address, seen }
}

/// A keeper that writes down what it is told.
#[derive(Default)]
struct Kept {
    renewed: Mutex<Vec<String>>,
    refused: AtomicUsize,
}

impl Keeper for Kept {
    fn renewed(&self, credential: &str) {
        self.renewed.lock().unwrap().push(credential.to_owned());
    }

    fn refused(&self) {
        self.refused.fetch_add(1, Ordering::SeqCst);
    }
}

const KEPT: &str =
    r#"kavita:{"token":"jwt.one","refreshToken":"refresh.one","apiKey":"key-one"}"#;

struct Kit {
    fake: Fake,
    kept: Arc<Kept>,
    flag: Arc<AtomicBool>,
    source: Arc<dyn kaeri_core::traits::BookSource>,
}

async fn kavita(plan: Plan) -> Kit {
    let fake = serve(plan).await;
    let kept = Arc::new(Kept::default());
    let flag = Arc::new(AtomicBool::new(false));
    let source = source_with(
        &reqwest::Client::new(),
        &Server {
            id: "k",
            name: "Kavita",
            kind: Kind::Kavita,
            address: &fake.address,
            credential: KEPT,
        },
        &[],
        std::env::temp_dir().join("kaeri-renewal-test"),
        Gate::new(flag.clone(), kept.clone()),
    )
    .unwrap();
    Kit { fake, kept, flag, source }
}

fn renewed(kit: &Kit) -> Vec<Tokens> {
    kit.kept.renewed.lock().unwrap().iter().map(|k| Tokens::parse(k).unwrap()).collect()
}

#[tokio::test]
async fn a_lapsed_jwt_is_refreshed_once_and_the_rotated_tokens_are_kept() {
    let kit = kavita(Plan::rotating()).await;
    let hints = BookHints::default();
    let chapters = kit.source.chapters("7", &hints).await.unwrap();
    assert_eq!(chapters.len(), 2);

    let refreshes = kit.fake.requests("/api/Account/refresh-token");
    assert_eq!(refreshes.len(), 1);
    let body: serde_json::Value = serde_json::from_str(&refreshes[0].3).unwrap();
    assert_eq!(body, serde_json::json!({ "token": "jwt.one", "refreshToken": "refresh.one" }));
    assert_eq!(refreshes[0].2, "", "the refresh is asked without the lapsed JWT");
    // The refused call went again with the new JWT.
    let details = kit.fake.requests("/api/Series/series-detail");
    let auths: Vec<&str> = details.iter().map(|r| r.2.as_str()).collect();
    assert_eq!(auths, ["Bearer jwt.one", "Bearer jwt.two"]);

    // The rotated refresh token is what is kept, with the key as it was.
    let fresh = Tokens {
        token: "jwt.two".into(),
        refresh: "refresh.two".into(),
        api_key: "key-one".into(),
    };
    assert_eq!(renewed(&kit), [fresh]);
    assert!(!kit.flag.load(Ordering::SeqCst));
    assert_eq!(kit.kept.refused.load(Ordering::SeqCst), 0);

    // A download and a page after it carry the new JWT and renew nothing.
    let ChapterContent::Images(pages) = kit.source.content("7", "901").await.unwrap() else {
        panic!("images");
    };
    assert_eq!(kit.source.fetch_page(&pages[0]).await.unwrap(), b"image");
    assert_eq!(kit.fake.count("/api/Account/refresh-token"), 1);
    assert!(kit.source.is_available().is_ok());
}

#[tokio::test]
async fn a_refresh_turned_down_falls_back_to_the_api_key() {
    let kit = kavita(Plan { refresh: None, key: Some("jwt.key"), ..Plan::rotating() }).await;
    kit.source.chapters("7", &BookHints::default()).await.unwrap();
    assert_eq!(kit.fake.count("/api/Account/refresh-token"), 1);
    assert_eq!(kit.fake.count("/api/Plugin/authenticate"), 1);
    let fresh = Tokens {
        token: "jwt.key".into(),
        refresh: "refresh.key".into(),
        api_key: "key-one".into(),
    };
    assert_eq!(renewed(&kit), [fresh]);
    assert!(!kit.flag.load(Ordering::SeqCst));
}

#[tokio::test]
async fn a_renewal_turned_down_signs_the_source_out_once() {
    let kit = kavita(Plan { refresh: None, key: None, ..Plan::rotating() }).await;
    let hints = BookHints::default();
    let error = kit.source.chapters("7", &hints).await.unwrap_err();
    assert!(matches!(error, ProviderError::Unavailable(_)), "{error:?}");
    assert!(kit.flag.load(Ordering::SeqCst));
    assert!(matches!(kit.source.is_available(), Err(ProviderError::Unavailable(_))));
    assert_eq!(kit.kept.refused.load(Ordering::SeqCst), 1);
    assert!(renewed(&kit).is_empty());

    // A download refused after it is a sign-in to redo too, and asks for no second renewal.
    let error = kit.source.content("7", "901").await.unwrap_err();
    assert!(matches!(error, ProviderError::Unavailable(_)), "{error:?}");
    assert_eq!(kit.fake.count("/api/Account/refresh-token"), 1);
    assert_eq!(kit.fake.count("/api/Plugin/authenticate"), 1);
    assert_eq!(kit.kept.refused.load(Ordering::SeqCst), 1, "the keeper is told once");
}

#[tokio::test]
async fn calls_refused_at_once_share_one_refresh() {
    let kit =
        kavita(Plan { refresh_wait: Duration::from_millis(300), ..Plan::rotating() }).await;
    let hints = BookHints::default();
    let page = kaeri_core::books::PageRef::new(format!(
        "{}/api/Reader/image?chapterId=901&page=0&apiKey=key-one",
        kit.fake.address
    ));
    let lists = (0..4).map(|_| kit.source.chapters("7", &hints));
    let pages = (0..4).map(|_| kit.source.fetch_page(&page));
    let (lists, pages) =
        tokio::join!(futures::future::join_all(lists), futures::future::join_all(pages));
    assert!(lists.iter().all(Result::is_ok), "{lists:?}");
    assert!(pages.iter().all(|p| p.as_deref() == Ok(b"image".as_slice())), "{pages:?}");
    assert_eq!(kit.fake.count("/api/Account/refresh-token"), 1);
    assert_eq!(renewed(&kit).len(), 1);
    assert!(!kit.flag.load(Ordering::SeqCst));
}

#[tokio::test]
async fn a_server_that_cannot_answer_the_refresh_does_not_sign_out() {
    let kit = kavita(Plan { refresh_status: Some(503), ..Plan::rotating() }).await;
    let error = kit.source.chapters("7", &BookHints::default()).await.unwrap_err();
    assert!(!matches!(error, ProviderError::Unavailable(_)), "{error:?}");
    assert!(!kit.flag.load(Ordering::SeqCst));
    assert_eq!(kit.kept.refused.load(Ordering::SeqCst), 0);
    assert_eq!(kit.fake.count("/api/Plugin/authenticate"), 0);
    assert!(kit.source.is_available().is_ok());
}

#[tokio::test]
async fn the_recheck_of_an_empty_server_renews_too() {
    let fake = serve(Plan::rotating()).await;
    let kept = Arc::new(Kept::default());
    let flag = Arc::new(AtomicBool::new(false));
    let gate = Gate::new(flag.clone(), kept.clone());
    let client = reqwest::Client::new();
    let counted = count_series_kept(&client, &fake.address, Kind::Kavita, KEPT, &gate).await;
    assert_eq!(counted, Ok(Some(212)));
    assert_eq!(kept.renewed.lock().unwrap().len(), 1);

    // Turned down, the recheck signs the server out as its source would.
    let fake = serve(Plan { refresh: None, key: None, ..Plan::rotating() }).await;
    let kept = Arc::new(Kept::default());
    let gate = Gate::new(flag.clone(), kept.clone());
    assert!(
        count_series_kept(&client, &fake.address, Kind::Kavita, KEPT, &gate).await.is_err()
    );
    assert!(flag.load(Ordering::SeqCst));
    assert_eq!(kept.refused.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_lapsed_komga_session_signs_the_source_out_once() {
    // Every path on the fake wants a Kavita JWT, so a Komga session token is refused.
    let fake = serve(Plan::rotating()).await;
    let kept = Arc::new(Kept::default());
    let flag = Arc::new(AtomicBool::new(false));
    let source = source(
        &reqwest::Client::new(),
        &Server {
            id: "m",
            name: "Komga",
            kind: Kind::Komga,
            address: &fake.address,
            credential: "token:lapsed-session",
        },
        std::env::temp_dir().join("kaeri-renewal-test"),
        Gate::new(flag.clone(), kept.clone()),
    )
    .unwrap();
    let hints = BookHints::default();
    assert!(matches!(source.chapters("s1", &hints).await, Err(ProviderError::Unavailable(_))));
    assert!(matches!(source.content("s1", "b1").await, Err(ProviderError::Unavailable(_))));
    assert!(matches!(source.is_available(), Err(ProviderError::Unavailable(_))));
    assert_eq!(kept.refused.load(Ordering::SeqCst), 1);
    assert!(kept.renewed.lock().unwrap().is_empty(), "a Komga session has no way to renew");
    assert_eq!(fake.count("/api/Account/refresh-token"), 0);
}
