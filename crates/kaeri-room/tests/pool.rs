//! The relay pool against three relays on loopback: never a public relay.

#![cfg(feature = "testing")]

use std::time::Duration;

use kaeri_room::{
    fake_relay::FakeRelay,
    keys::{Identity, RoomKeys},
    link::{Heard, Link, Out},
    pool::{self, Options},
    wire::{Body, Message, Status, VERSION},
};
use tokio::time::timeout;

const SECRET: [u8; 16] = [3; 16];

async fn start(n: usize) -> Vec<FakeRelay> {
    let mut out = Vec::new();
    for _ in 0..n {
        out.push(FakeRelay::start().await);
    }
    out
}

fn urls(relays: &[FakeRelay]) -> Vec<String> {
    relays.iter().map(FakeRelay::url).collect()
}

fn options() -> Options {
    Options { check_clock: false, ..Options::default() }
}

async fn join(secret: [u8; 16], relays: &[FakeRelay]) -> Link {
    pool::open_with(secret, urls(relays), options()).await.expect("a relay answers")
}

struct Member {
    link: Link,
    id: Identity,
    ctr: u64,
}

impl Member {
    async fn new(secret: [u8; 16], relays: &[FakeRelay]) -> Self {
        let link = join(secret, relays).await;
        let mut m = Self { link, id: Identity::new(), ctr: 0 };
        // Wait for every relay before speaking, so all three carry it.
        m.wait_up(relays.len() as u32).await;
        m
    }

    fn body(&mut self, m: Message) -> Body {
        self.ctr += 1;
        Body {
            v: VERSION,
            from: self.link.session.pubkey(),
            mid: self.id.mid(),
            ctr: self.ctr,
            t: 0,
            st: Status::default(),
            m,
        }
    }

    fn hello(&mut self) {
        let m = Message::Hello {
            name: "Mika".into(),
            app: "0.6".into(),
            caps: vec![],
            idpk: self.id.pubkey(),
            idsig: self.id.prove(&self.link.session.pubkey(), &self.link.room_id),
        };
        let body = self.body(m);
        self.link.out.send(Out::Body(body)).unwrap();
    }

    fn say(&mut self, m: Message) {
        let body = self.body(m);
        self.link.out.send(Out::Body(body)).unwrap();
    }

    async fn next(&mut self, want: impl Fn(&Heard) -> bool) -> Option<Heard> {
        timeout(Duration::from_secs(5), async {
            loop {
                let heard = self.link.heard.recv().await?;
                if want(&heard) {
                    return Some(heard);
                }
            }
        })
        .await
        .ok()
        .flatten()
    }

    async fn wait_up(&mut self, n: u32) {
        self.next(|h| matches!(h, Heard::Up { relays_up } if *relays_up >= n))
            .await
            .expect("relays up");
    }

    /// Bodies heard in the next `ms`.
    async fn drain(&mut self, ms: u64) -> Vec<Body> {
        let mut out = Vec::new();
        let _ = timeout(Duration::from_millis(ms), async {
            while let Some(h) = self.link.heard.recv().await {
                if let Heard::Body(b) = h {
                    out.push(*b);
                }
            }
        })
        .await;
        out
    }
}

fn is_chat(text: &'static str) -> impl Fn(&Heard) -> bool {
    move |h| matches!(h, Heard::Body(b) if matches!(&b.m, Message::Chat { text: t, .. } if t == text))
}

#[tokio::test]
async fn two_members_hear_each_other_once_through_three_relays() {
    let relays = start(3).await;
    let mut ada = Member::new(SECRET, &relays).await;
    let mut ren = Member::new(SECRET, &relays).await;
    ada.hello();
    ren.next(|h| matches!(h, Heard::Body(b) if matches!(b.m, Message::Hello { .. })))
        .await
        .expect("Ren hears Ada's Hello");
    ada.say(Message::Chat { id: "c1".into(), text: "hi".into() });
    let bodies = ren.drain(800).await;
    let chats = bodies.iter().filter(|b| matches!(b.m, Message::Chat { .. })).count();
    assert_eq!(chats, 1, "first copy only: {bodies:?}");
    // Ada hears her own echo once too, and each relay measured a round trip.
    let own = ada.drain(300).await;
    assert_eq!(own.iter().filter(|b| matches!(b.m, Message::Chat { .. })).count(), 1);
    // Every relay carried the event.
    for relay in &relays {
        assert!(relay.events().len() >= 2);
    }
}

#[tokio::test]
async fn relays_see_nothing_and_outsiders_hear_nothing() {
    let relays = start(1).await;
    let mut ada = Member::new(SECRET, &relays).await;
    let mut outsider = Member::new([9; 16], &relays).await;
    ada.hello();
    ada.say(Message::Chat { id: "c1".into(), text: "the secret word".into() });
    assert!(outsider.drain(800).await.is_empty());
    for event in relays[0].events() {
        let json = serde_json::to_string(&event).unwrap();
        // A phrase with a space can't be in base64; a short name is looked for outside the
        // sealed body, which random base64 can spell by chance.
        assert!(!json.contains("secret word"), "{json}");
        let mut open = serde_json::to_value(&event).unwrap();
        open["content"] = serde_json::Value::Null;
        assert!(!open.to_string().contains("Mika"), "{open}");
        assert_eq!(event.kind.as_u16(), kaeri_room::envelope::KIND);
        let tag = RoomKeys::new(SECRET).send_tag(event.created_at.as_secs() as i64 * 1000);
        assert!(json.contains(&tag));
    }
}

#[tokio::test]
async fn a_stranger_cant_speak_as_a_member() {
    let relays = start(1).await;
    let mut ada = Member::new(SECRET, &relays).await;
    let mut ren = Member::new(SECRET, &relays).await;
    let mut stranger = Member::new(SECRET, &relays).await;
    ada.hello();
    ren.next(|h| matches!(h, Heard::Body(b) if matches!(b.m, Message::Hello { .. })))
        .await
        .unwrap();
    // The stranger holds the room key but claims Ada's mid with its own identity.
    stranger.id = Identity::new();
    let ada_mid = ada.id.mid();
    let mut fake = stranger.body(Message::Leave);
    fake.mid = ada_mid;
    stranger.link.out.send(Out::Body(fake)).unwrap();
    let heard = ren.drain(800).await;
    assert!(!heard.iter().any(|b| matches!(b.m, Message::Leave)), "{heard:?}");
}

#[tokio::test]
async fn a_rate_limited_relay_gets_no_heartbeats_and_the_room_holds_on_the_others() {
    let relays = start(3).await;
    let mut ada = Member::new(SECRET, &relays).await;
    let mut ren = Member::new(SECRET, &relays).await;
    ada.hello();
    ren.next(|h| matches!(h, Heard::Body(b) if matches!(b.m, Message::Hello { .. })))
        .await
        .unwrap();
    relays[0].refuse(Some("rate-limited: slow down"), Some(1));
    ada.say(Message::Status);
    tokio::time::sleep(Duration::from_millis(300)).await;
    let before = relays[0].events().len();
    for _ in 0..3 {
        ada.say(Message::Heartbeat);
    }
    ada.say(Message::Chat { id: "c".into(), text: "still here".into() });
    ren.next(is_chat("still here")).await.expect("the other relays carry it");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        relays[0].events().len(),
        before,
        "no heartbeat or chat to a rate-limited relay"
    );
    // A change still goes to it.
    ada.say(Message::Status);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(relays[0].events().len(), before + 1);
}

#[tokio::test]
async fn the_send_cap_drops_heartbeats_and_holds_changes() {
    let relays = start(1).await;
    let mut ada = Member::new(SECRET, &relays).await;
    ada.hello();
    for _ in 0..10 {
        ada.say(Message::Heartbeat);
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
    // Six in ten seconds: the Hello and five heartbeats; the rest dropped.
    assert_eq!(relays[0].events().len(), 6);
    ada.say(Message::Status);
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(relays[0].events().len(), 6, "a change waits for room under the cap");
}

#[tokio::test]
async fn auth_required_is_answered_and_the_event_sent_again() {
    let relays = start(1).await;
    relays[0].require_auth(true);
    let mut ada = Member::new(SECRET, &relays).await;
    let mut ren = Member::new(SECRET, &relays).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    ada.hello();
    ren.next(|h| matches!(h, Heard::Body(b) if matches!(b.m, Message::Hello { .. })))
        .await
        .expect("the Hello went through after AUTH, on a subscription made after AUTH");
}

#[tokio::test]
async fn restricted_after_auth_leaves_the_relay_unusable() {
    let relays = start(2).await;
    relays[0].require_auth(true);
    relays[0].restrict(true);
    let mut ada = Member { link: join(SECRET, &relays).await, id: Identity::new(), ctr: 0 };
    ada.next(|h| matches!(h, Heard::Relays(list) if !list[0].healthy && list[1].healthy))
        .await
        .expect("relay 0 marked unhealthy");
    ada.next(|h| matches!(h, Heard::Up { relays_up: 1 })).await.expect("one relay up");
}

#[tokio::test]
async fn a_hang_up_reconnects_and_resubscribes() {
    let relays = start(1).await;
    let mut ada = Member::new(SECRET, &relays).await;
    let mut ren = Member::new(SECRET, &relays).await;
    ada.hello();
    ren.next(|h| matches!(h, Heard::Body(b) if matches!(b.m, Message::Hello { .. })))
        .await
        .unwrap();
    relays[0].hang_up();
    ren.next(|h| matches!(h, Heard::Up { relays_up: 0 })).await.expect("down");
    ren.next(|h| matches!(h, Heard::Up { relays_up: 1 })).await.expect("back after 1 s");
    ada.wait_up(1).await;
    ada.say(Message::Chat { id: "c2".into(), text: "back".into() });
    ren.next(is_chat("back")).await.expect("subscribed again");
}

#[tokio::test]
async fn no_relay_answers_and_the_join_says_so() {
    let dead = FakeRelay::start().await;
    dead.down(true);
    let started = tokio::time::Instant::now();
    let result = pool::open_with(SECRET, vec![dead.url()], options()).await;
    assert!(result.is_err());
    assert!(started.elapsed() >= Duration::from_secs(8));
}

#[tokio::test]
async fn the_spares_are_tried_when_the_links_relays_are_gone() {
    let dead = FakeRelay::start().await;
    dead.down(true);
    let spare = FakeRelay::start().await;
    let link = pool::open_with(
        SECRET,
        vec![dead.url()],
        Options { spares: vec![spare.url()], ..options() },
    )
    .await
    .expect("the spare answers");
    assert_eq!(link.relays, vec![spare.url()]);
}

#[tokio::test]
async fn the_background_flush_is_answered() {
    let relays = start(1).await;
    let mut ada = Member::new(SECRET, &relays).await;
    ada.hello();
    let body = ada.body(Message::Status);
    ada.link.out.send(Out::Flush(body)).unwrap();
    ada.next(|h| matches!(h, Heard::Flushed)).await.expect("flushed on the relay's OK");
    // A relay that never answers: flushed after 2 s anyway.
    relays[0].down(true);
    let body = ada.body(Message::Status);
    ada.link.out.send(Out::Flush(body)).unwrap();
    ada.next(|h| matches!(h, Heard::Flushed)).await.expect("flushed after the limit");
}

#[tokio::test]
async fn a_rekey_moves_everyone_present_to_the_new_tag() {
    let relays = start(1).await;
    let mut ada = Member::new(SECRET, &relays).await;
    let mut ren = Member::new(SECRET, &relays).await;
    ada.hello();
    ren.next(|h| matches!(h, Heard::Body(b) if matches!(b.m, Message::Hello { .. })))
        .await
        .unwrap();
    let next = [5; 16];
    ada.link.out.send(Out::Rekey(next)).unwrap();
    ren.link.out.send(Out::Rekey(next)).unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    ada.say(Message::Chat { id: "c3".into(), text: "new key".into() });
    ren.next(is_chat("new key")).await.expect("heard under the new key");
    let last = relays[0].events().last().cloned().unwrap();
    let tag = RoomKeys::new(next).send_tag(last.created_at.as_secs() as i64 * 1000);
    assert!(serde_json::to_string(&last).unwrap().contains(&tag));
    // Someone left on the old key still hears nothing new, and isn't heard after two minutes.
    let mut late = Member::new(SECRET, &relays).await;
    ada.say(Message::Chat { id: "c4".into(), text: "after".into() });
    assert!(late.drain(500).await.is_empty());
}

#[tokio::test]
async fn the_clock_moves_only_when_two_relays_agree() {
    let relays = start(3).await;
    relays[0].date(-90, None);
    relays[1].date(-85, Some(0));
    let link = pool::open_with(SECRET, urls(&relays), Options::default()).await.unwrap();
    timeout(Duration::from_secs(5), async {
        while link.offset.load(std::sync::atomic::Ordering::Relaxed) == 0 {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("corrected");
    let off = link.offset.load(std::sync::atomic::Ordering::Relaxed);
    assert!((-92_000..=-83_000).contains(&off), "{off}");

    let lone = start(3).await;
    lone[0].date(-3600, None);
    let link = pool::open_with(SECRET, urls(&lone), Options::default()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert_eq!(link.offset.load(std::sync::atomic::Ordering::Relaxed), 0);
}
