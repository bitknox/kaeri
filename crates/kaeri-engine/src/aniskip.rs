//! Sending the viewer's marks and votes to AniSkip, with sharing on.
//!
//! A playback queues what a save or a report sends in the store's AniSkip outbox and asks for
//! it to be sent; the engine asks too at launch. What is sent and what AniSkip's answers mean
//! is `kaeri_playback::aniskip`'s. This sends what is due, one at a time, and keeps what
//! could not be sent for later: offline, AniSkip down, or over its limits.
//!
//! The app is named to AniSkip as [`kaeri_core::APP_NAME`], and the install by a random id
//! made the first time anything is sent and kept in the store.

use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
};

use kaeri_net::HttpClient;
use kaeri_playback::{
    aniskip::{self, Outcome, Request},
    skip,
};
use kaeri_store::Store;

/// Where the store keeps the install's id for AniSkip.
const SUBMITTER: &str = "aniskip_submitter";

/// How many requests one pass sends at most. AniSkip takes ten submissions a day per episode,
/// so a pass never needs more.
const PASS: u32 = 20;

/// How long a pass waits for the next request due, at most, in seconds. Past this the request
/// waits for the next save, report or launch.
const WAITS_UP_TO: i64 = 30 * 60;

/// What one pass did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pass {
    pub sent: u32,
    pub later: u32,
    pub dropped: u32,
}

/// The sender, with the address it sends to.
pub struct Sharing {
    origin: String,
    sending: AtomicBool,
}

impl Sharing {
    /// The process's own, which sends to AniSkip.
    pub fn shared() -> &'static Self {
        static SHARED: OnceLock<Sharing> = OnceLock::new();
        SHARED.get_or_init(|| Self::on(skip::API))
    }

    /// One that sends to `origin` instead: a test's server.
    pub fn on(origin: &str) -> Self {
        Self { origin: origin.to_owned(), sending: AtomicBool::new(false) }
    }

    /// Send what is due, in the background, and what falls due in the next half hour after
    /// it. A second ask while a pass runs is the same pass.
    pub fn kick(&'static self, http: HttpClient, store: Store) {
        if self.sending.swap(true, Ordering::AcqRel) {
            return;
        }
        tokio::spawn(async move {
            loop {
                let now = kaeri_store::now();
                let pass = self.send_due(&http, &store, now).await;
                tracing::debug!(?pass, "aniskip outbox pass");
                let next = store.next_aniskip_due().ok().flatten();
                match next {
                    Some(at) if at <= now + WAITS_UP_TO && pass.sent + pass.later > 0 => {
                        let wait = u64::try_from((at - now).max(1)).unwrap_or(1);
                        tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                    }
                    _ => break,
                }
            }
            self.sending.store(false, Ordering::Release);
        });
    }

    /// Send what is due at `now`, oldest first, and keep each answer. With no network the
    /// pass stops at the first request, which is not counted as tried: the rest would fail
    /// the same way.
    pub async fn send_due(&self, http: &HttpClient, store: &Store, now: i64) -> Pass {
        let mut pass = Pass::default();
        let due = match store.due_aniskip(now, PASS) {
            Ok(due) if !due.is_empty() => due,
            Ok(_) => return pass,
            Err(e) => {
                tracing::warn!(error = %e, "aniskip outbox unread");
                return pass;
            }
        };
        let submitter = match submitter(store) {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!(error = %e, "no id for aniskip");
                return pass;
            }
        };
        for item in due {
            let Ok(request) = serde_json::from_str::<Request>(&item.request) else {
                // Kept by a build that wrote it another way: not to be sent as it stands.
                let _ = store.aniskip_failed(item.id);
                pass.dropped += 1;
                continue;
            };
            let answer = http
                .plain()
                .post(request.url_on(&self.origin))
                .header("content-type", "application/json")
                .body(request.body(kaeri_core::APP_NAME, &submitter).to_string())
                .send()
                .await;
            let (status, body) = match answer {
                Ok(response) => {
                    let status = response.status().as_u16();
                    (Some(status), response.text().await.unwrap_or_default())
                }
                Err(e) => {
                    tracing::debug!(error = %e, op = request.op(), "aniskip not reached");
                    break;
                }
            };
            let kept = match aniskip::outcome(&request, status, now, item.attempts + 1) {
                Outcome::Sent => {
                    pass.sent += 1;
                    if let (Request::Submit(_), Some((show, episode, kind)), Some(skip_id)) =
                        (&request, &item.mark, aniskip::submitted_id(&body))
                    {
                        let _ = store.set_skip_mark_id(*show, episode, kind, &skip_id);
                    }
                    store.aniskip_sent(item.id)
                }
                Outcome::Later(at) => {
                    pass.later += 1;
                    store.aniskip_later(item.id, at)
                }
                Outcome::Dropped => {
                    tracing::info!(?status, op = request.op(), "aniskip request let go");
                    pass.dropped += 1;
                    store.aniskip_failed(item.id)
                }
            };
            if let Err(e) = kept {
                tracing::warn!(error = %e, "aniskip answer not kept");
            }
        }
        pass
    }
}

/// The install's id for AniSkip, made the first time it is asked for.
pub fn submitter(store: &Store) -> kaeri_store::Result<String> {
    if let Some(id) = store.get_meta_text(SUBMITTER)?.filter(|id| !id.is_empty()) {
        return Ok(id);
    }
    let id = uuid_v4(rand::random::<u128>());
    store.set_meta_text(SUBMITTER, &id)?;
    Ok(id)
}

/// `bits` as a random (version 4) UUID, as AniSkip asks for.
fn uuid_v4(bits: u128) -> String {
    let bits = (bits & !(0xf << 76)) | (0x4 << 76);
    let bits = (bits & !(0x3 << 62)) | (0x2 << 62);
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        bits >> 96,
        (bits >> 80) & 0xffff,
        (bits >> 64) & 0xffff,
        (bits >> 48) & 0xffff,
        bits & 0xffff_ffff_ffff,
    )
}

/// Sharing was turned on or off in Settings, or answered in the player. On, what waits is
/// sent; off, it is forgotten, so nothing is sent with sharing off.
pub fn sharing_changed(share: Option<bool>, http: &HttpClient, store: &Store) {
    if share == Some(true) {
        Sharing::shared().kick(http.clone(), store.clone());
    } else if let Err(e) = store.clear_aniskip() {
        tracing::warn!(error = %e, "aniskip outbox not cleared");
    }
}

/// Queue `requests` and ask for them to be sent. `mark` is the show, episode and kind of the
/// mark a submission among them came from.
pub fn queue(
    http: &HttpClient,
    store: &Store,
    requests: &[Request],
    mark: Option<(kaeri_core::ids::AnilistId, &str)>,
) {
    if requests.is_empty() {
        return;
    }
    let now = kaeri_store::now();
    for request in requests {
        let Ok(json) = serde_json::to_string(request) else { continue };
        let names = match (request, mark) {
            (Request::Submit(submission), Some((show, episode))) => {
                Some((show, episode, submission.kind.as_str()))
            }
            _ => None,
        };
        if let Err(e) = store.queue_aniskip(&json, names, now) {
            tracing::warn!(error = %e, "aniskip request not queued");
        }
    }
    Sharing::shared().kick(http.clone(), store.clone());
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use kaeri_core::ids::AnilistId;
    use kaeri_playback::aniskip::{Submission, Vote};
    use kaeri_playback::skip::SkipKind;
    use kaeri_store::SkipMark;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    const SHOW: AnilistId = AnilistId::new(900_303);

    /// What the fake AniSkip was sent: the request line and the body.
    type Heard = Arc<Mutex<Vec<(String, serde_json::Value)>>>;

    /// A fake AniSkip on a local port that answers each request with the next of `answers`,
    /// a status and a body, and notes what it was sent.
    async fn fake_aniskip(answers: Vec<(u16, &'static str)>) -> (String, Heard) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let heard: Heard = Arc::default();
        let noted = heard.clone();
        tokio::spawn(async move {
            for (status, body) in answers {
                let Ok((mut socket, _)) = listener.accept().await else { return };
                let request = read_request(&mut socket).await;
                noted.lock().unwrap().push(request);
                let reply = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes()).await;
                let _ = socket.shutdown().await;
            }
        });
        (origin, heard)
    }

    async fn read_request(socket: &mut tokio::net::TcpStream) -> (String, serde_json::Value) {
        let mut raw = Vec::new();
        let mut chunk = [0_u8; 4096];
        loop {
            let read = socket.read(&mut chunk).await.unwrap_or(0);
            if read == 0 {
                break;
            }
            raw.extend_from_slice(&chunk[..read]);
            let text = String::from_utf8_lossy(&raw);
            if let Some(split) = text.find("\r\n\r\n") {
                let length = text[..split]
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                if raw.len() >= split + 4 + length {
                    break;
                }
            }
        }
        let text = String::from_utf8_lossy(&raw).into_owned();
        let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
        let line = head.lines().next().unwrap_or_default().to_owned();
        (line, serde_json::from_str(body).unwrap_or(serde_json::Value::Null))
    }

    fn submission() -> Request {
        Request::Submit(Submission::new(4_242, 3, SkipKind::Opening, (61.5, 151.5), 1_420.0))
    }

    fn vote(up: bool) -> Request {
        Request::Vote(Vote { skip_id: "time-1".into(), up })
    }

    fn store_with(requests: &[Request]) -> Store {
        let store = Store::open_in_memory().unwrap();
        store
            .put_skip_mark(&SkipMark {
                anilist_id: SHOW,
                episode: "3".into(),
                kind: "op".into(),
                start_ms: 61_500,
                end_ms: 151_500,
                length_ms: 1_420_000,
                marked_at: 1,
                skip_id: None,
            })
            .unwrap();
        for request in requests {
            let mark = matches!(request, Request::Submit(_)).then_some((SHOW, "3", "op"));
            store.queue_aniskip(&serde_json::to_string(request).unwrap(), mark, 0).unwrap();
        }
        store
    }

    #[tokio::test]
    async fn a_submission_and_a_vote_reach_aniskip_with_the_apps_name() {
        let (origin, heard) =
            fake_aniskip(vec![(200, r#"{"message":"ok","skipId":"time-9"}"#), (200, "{}")])
                .await;
        let store = store_with(&[submission(), vote(false)]);
        let http = HttpClient::offline(&kaeri_core::config::NetworkConfig::default()).unwrap();
        let pass = Sharing::on(&origin).send_due(&http, &store, 10).await;
        assert_eq!(pass, Pass { sent: 2, later: 0, dropped: 0 });

        let heard = heard.lock().unwrap().clone();
        assert_eq!(heard[0].0, "POST /v2/skip-times/4242/3 HTTP/1.1");
        assert_eq!(heard[0].1["providerName"], kaeri_core::APP_NAME);
        assert_eq!(heard[0].1["skipType"], "op");
        let id = heard[0].1["submitterId"].as_str().unwrap().to_owned();
        assert_eq!(id, submitter(&store).unwrap(), "the same id each time");
        assert_eq!(heard[1].0, "POST /v2/skip-times/vote/time-1 HTTP/1.1");
        assert_eq!(heard[1].1, serde_json::json!({ "voteType": "downvote" }));

        // The mark knows the time it became, and nothing waits.
        assert_eq!(store.skip_marks(SHOW, "3").unwrap()[0].skip_id.as_deref(), Some("time-9"));
        assert_eq!(store.next_aniskip_due().unwrap(), None);
    }

    #[tokio::test]
    async fn too_many_waits_and_a_refusal_is_let_go() {
        let (origin, _heard) = fake_aniskip(vec![(429, "{}"), (429, "{}"), (400, "{}")]).await;
        let store = store_with(&[submission(), vote(true), vote(false)]);
        let http = HttpClient::offline(&kaeri_core::config::NetworkConfig::default()).unwrap();
        let now = 86_400 * 100 + 50;
        let pass = Sharing::on(&origin).send_due(&http, &store, now).await;
        assert_eq!(pass, Pass { sent: 0, later: 2, dropped: 1 });
        assert_eq!(store.aniskip_failed_count().unwrap(), 1);
        // The vote waits an hour, the submission the next UTC day.
        let due: Vec<_> = store.due_aniskip(i64::MAX, 10).unwrap();
        assert_eq!(due.len(), 2);
        assert!(store.due_aniskip(now + aniskip::VOTE_WAIT - 1, 10).unwrap().is_empty());
        assert_eq!(store.due_aniskip(now + aniskip::VOTE_WAIT, 10).unwrap().len(), 1);
        assert_eq!(store.due_aniskip(86_400 * 101, 10).unwrap().len(), 2);
    }

    #[tokio::test]
    async fn offline_nothing_counts_as_tried() {
        // A port nothing listens on.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        let store = store_with(&[submission(), vote(true)]);
        let http = HttpClient::offline(&kaeri_core::config::NetworkConfig::default()).unwrap();
        let pass = Sharing::on(&origin).send_due(&http, &store, 10).await;
        assert_eq!(pass, Pass::default());
        let due = store.due_aniskip(10, 10).unwrap();
        assert_eq!(due.iter().map(|i| i.attempts).collect::<Vec<_>>(), [0, 0]);
    }

    #[test]
    fn the_install_id_is_a_random_uuid() {
        let id = uuid_v4(u128::MAX);
        assert_eq!(id, "ffffffff-ffff-4fff-bfff-ffffffffffff");
        assert_eq!(uuid_v4(0), "00000000-0000-4000-8000-000000000000");
    }
}
