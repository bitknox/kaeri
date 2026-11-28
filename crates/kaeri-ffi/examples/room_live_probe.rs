//! A room on real public relays, driven through the engine's own room as the apps drive it, to
//! see whether the relays Kaeri ships with carry it and how fast. Never run by CI: it talks to
//! other people's servers, so keep runs few, small and apart (each sends well under 100 events).
//!
//! ```text
//! cargo run -p kaeri-ffi --features testing,room --example room_live_probe -- \
//!     [--relays wss://a,wss://b] [--spares wss://c] [--report out.md] [--linger S] [--fake]
//! ```
//!
//! Without `--relays` the room is on the built-in relays (the invite then names none), and
//! without `--spares` on the built-in spares. Each
//! member is its own engine on its own temporary home, with its own identity; run it under the
//! HOME guard all the same. Everything said in the room is sealed; the names and the episode
//! are made up.
//!
//! The script: Ada starts a room; Ren and Mika join by the invite; a line of chat from each;
//! Ada plays and the room waits for those still loading until they say Don't wait for me; Ren
//! pauses; Mika skips the opening (a seek for everyone); Ada makes a new invite and Kai joins by
//! it; Ren goes to the background and back, and says something; Mika leaves; Ada leaves and the
//! room closes. Each step is timed as the members' apps see it, and every event on the wire is
//! followed from its send to each member over each relay, from the relay connections' debug log
//! (`kaeri_room::wire`), with whatever the relays answered.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fmt::Write as _,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use kaeri_core::ids::AnilistId;
use kaeri_ffi::{FfiDirs, FfiEngine, FfiHost, FfiStart};
use kaeri_model::{
    command::{AppState, Command},
    event::Event,
    room::{MemberState, RoomChatBody, RoomCommand, RoomEpisode, RoomMessageKind, RoomView},
};
use tracing::field::{Field, Visit};
use tracing_subscriber::{Layer, layer::SubscriberExt};

/// What a step waits for in one member's app: an event, with the room view as it was then.
type Done = Box<dyn Fn(&Event, Option<&RoomView>) -> bool>;
/// An event's first send, its sender's session, and the relays it went to.
type Sent = (Instant, String, BTreeSet<String>);

/// The longest any one step may take before it counts as failed.
const STEP: Duration = Duration::from_secs(30);

// ── the relay connections' log ───────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct Rec {
    at: Instant,
    msg: String,
    f: BTreeMap<String, String>,
}

impl Rec {
    fn get(&self, k: &str) -> &str {
        self.f.get(k).map_or("", String::as_str)
    }
}

#[derive(Default)]
struct Fields(BTreeMap<String, String>, String);

impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let v = format!("{value:?}");
        if field.name() == "message" {
            self.1 = v;
        } else {
            self.0.insert(field.name().to_owned(), v);
        }
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.1 = value.to_owned();
        } else {
            self.0.insert(field.name().to_owned(), value.to_owned());
        }
    }
}

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<Rec>>>);

impl<S: tracing::Subscriber> Layer<S> for Log {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if !event.metadata().target().starts_with("kaeri_room") {
            return;
        }
        let mut f = Fields::default();
        event.record(&mut f);
        self.0.lock().unwrap().push(Rec { at: Instant::now(), msg: f.1, f: f.0 });
    }
}

// ── the members ──────────────────────────────────────────────────────────────────────────

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

/// One member: an engine on its own home, with everything it said to its app.
struct Member {
    name: &'static str,
    engine: Arc<FfiEngine>,
    events: Arc<Mutex<Vec<(Instant, Event)>>>,
    view: Arc<Mutex<Option<RoomView>>>,
    _home: tempfile::TempDir,
}

impl Member {
    fn new(name: &'static str) -> Self {
        let home = tempfile::tempdir().expect("a temporary home");
        let dir = |n: &str| home.path().join(n).display().to_string();
        let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
        let engine = FfiEngine::start_testing(dirs, Arc::new(Quiet), FfiStart::default())
            .expect("the engine starts");
        let events = Arc::new(Mutex::new(Vec::new()));
        let view = Arc::new(Mutex::new(None));
        {
            let (engine, events, view) =
                (Arc::clone(&engine), Arc::clone(&events), Arc::clone(&view));
            thread::spawn(move || {
                loop {
                    let batch = futures::executor::block_on(engine.next_events());
                    if batch.is_empty() {
                        return;
                    }
                    let now = Instant::now();
                    for event in batch {
                        if let Event::RoomJoined(v) | Event::RoomState(v) = &event {
                            *view.lock().unwrap() = Some(v.clone());
                        }
                        events.lock().unwrap().push((now, event));
                    }
                }
            });
        }
        Self { name, engine, events, view, _home: home }
    }

    fn room(&self, command: RoomCommand) {
        self.engine.send(Command::Room(command));
    }

    fn mark(&self) -> usize {
        self.events.lock().unwrap().len()
    }

    /// From event `from` on, until `done` holds on one (with the room view as it was then):
    /// how long after `since` that came, or `None` within [`STEP`].
    fn wait(
        &self,
        from: usize,
        since: Instant,
        done: impl Fn(&Event, Option<&RoomView>) -> bool,
    ) -> Option<Duration> {
        let mut seen = from;
        let mut view: Option<RoomView> = None;
        // The view as of `from`: the last one before it.
        for (_, e) in self.events.lock().unwrap()[..from].iter().rev() {
            if let Event::RoomJoined(v) | Event::RoomState(v) = e {
                view = Some(v.clone());
                break;
            }
        }
        while since.elapsed() < STEP {
            {
                let events = self.events.lock().unwrap();
                for (at, e) in &events[seen..] {
                    if let Event::RoomJoined(v) | Event::RoomState(v) = e {
                        view = Some(v.clone());
                    }
                    if done(e, view.as_ref()) {
                        return Some(at.saturating_duration_since(since));
                    }
                }
                seen = events.len();
            }
            thread::sleep(Duration::from_millis(20));
        }
        None
    }

    fn link(&self) -> Option<String> {
        self.view.lock().unwrap().as_ref().and_then(|v| v.link.clone())
    }
}

fn key_of(link: &str) -> String {
    link.split(['#', '&']).find(|p| p.starts_with("k=")).unwrap_or_default().to_owned()
}

fn sees(name: &'static str) -> impl Fn(&Event, Option<&RoomView>) -> bool {
    move |e, _| matches!(e, Event::RoomMembers(m) if m.iter().any(|m| m.name == name && m.state != MemberState::Left))
}

fn gone(name: &'static str) -> impl Fn(&Event, Option<&RoomView>) -> bool {
    move |e, _| {
        matches!(e, Event::RoomMembers(m) if m.iter().any(|m| m.name == name && m.state == MemberState::Left)
            || !m.iter().any(|m| m.name == name))
    }
}

fn said(from: &'static str, text: String) -> impl Fn(&Event, Option<&RoomView>) -> bool {
    move |e, _| {
        matches!(e, Event::RoomChat(c) if c.lines.iter().any(|l| l.from == from
            && matches!(&l.body, RoomChatBody::Said { text: t } if *t == text)))
    }
}

// ── the run ──────────────────────────────────────────────────────────────────────────────

/// One step: its name, when it began and ended, and how long each member took to see it.
struct Step {
    name: String,
    began: Instant,
    seen: Vec<(String, Option<Duration>)>,
}

struct Run {
    log: Log,
    steps: Vec<Step>,
    /// Session pk8 → member name.
    who: HashMap<String, &'static str>,
    t0: Instant,
}

impl Run {
    /// Sessions that first sent during the step just run are `name`'s.
    fn learn(&mut self, name: &'static str, since: Instant) {
        for r in self.log.0.lock().unwrap().iter() {
            if r.at >= since && r.msg == "send" {
                self.who.entry(r.get("me").to_owned()).or_insert(name);
            }
        }
    }

    fn step(&mut self, name: &str, act: impl FnOnce(), waits: Vec<(&Member, Done)>) -> Instant {
        let marks: Vec<usize> = waits.iter().map(|(m, _)| m.mark()).collect();
        let began = Instant::now();
        act();
        let mut seen = Vec::new();
        for ((m, done), from) in waits.iter().zip(marks) {
            let took = m.wait(from, began, done);
            seen.push((m.name.to_owned(), took));
        }
        let line: Vec<String> = seen
            .iter()
            .map(|(n, t)| match t {
                Some(t) => format!("{n} {:.2}s", t.as_secs_f64()),
                None => format!("{n} NOT SEEN"),
            })
            .collect();
        println!("  {:<40} {}", name, line.join(", "));
        self.steps.push(Step { name: name.to_owned(), began, seen });
        began
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let list = |k: &str| {
        arg(k).map(|v| v.split(',').map(str::to_owned).filter(|s| !s.is_empty()).collect())
    };
    let mut relays: Option<Vec<String>> = list("--relays");
    // `--fake`: three relays on loopback, to try the probe itself before the real ones.
    let fake_runtime = tokio::runtime::Runtime::new().expect("a runtime");
    let _fakes: Vec<kaeri_room::fake_relay::FakeRelay> = if args.iter().any(|a| a == "--fake") {
        let fakes = fake_runtime.block_on(async {
            vec![
                kaeri_room::fake_relay::FakeRelay::start().await,
                kaeri_room::fake_relay::FakeRelay::start().await,
                kaeri_room::fake_relay::FakeRelay::start().await,
            ]
        });
        relays = Some(fakes.iter().map(kaeri_room::fake_relay::FakeRelay::url).collect());
        fakes
    } else {
        Vec::new()
    };
    // The built-in spares unless given (a testing engine's own are `.example` hosts).
    let spares: Option<Vec<String>> = list("--spares").or_else(|| {
        Some(kaeri_model::room::SPARE_RELAYS.iter().map(|s| (*s).to_owned()).collect())
    });
    let report = arg("--report");
    // Idle this long once everyone is in, so a relay that stays bad is swapped for a spare.
    let linger: u64 = arg("--linger").and_then(|s| s.parse().ok()).unwrap_or(0);

    let log = Log::default();
    let filter = tracing_subscriber::filter::Targets::new()
        .with_target("kaeri_room", tracing::Level::DEBUG);
    tracing::subscriber::set_global_default(
        tracing_subscriber::registry().with(log.clone().with_filter(filter)),
    )
    .expect("the probe's log");

    let shown: Vec<String> = relays.clone().unwrap_or_else(|| {
        kaeri_model::room::DEFAULT_RELAYS.iter().map(|s| (*s).to_owned()).collect()
    });
    println!("── relays: {}", shown.join(" "));
    if let Some(s) = &spares {
        println!("── spares: {}", s.join(" "));
    }
    let nip11 = nip11_all(&shown, spares.as_deref().unwrap_or_default());

    let mut run =
        Run { log: log.clone(), steps: Vec::new(), who: HashMap::new(), t0: Instant::now() };
    let ada = Member::new("Ada");
    let ren = Member::new("Ren");
    let mika = Member::new("Mika");
    let kai = Member::new("Kai");
    for m in [&ada, &ren, &mika, &kai] {
        if let Some(s) = &spares {
            m.room(RoomCommand::SetSpareRelays { relays: s.clone() });
        }
    }
    let pick = RoomEpisode { id: AnilistId::new(900_001), episode: "9".into() };
    let b = |f: Done| f;

    println!("── steps (time each member's app took to see it)");
    // Ada starts the room.
    let began = run.step(
        "Ada starts the room",
        || {
            if let Some(r) = &relays {
                ada.room(RoomCommand::SetRelays { relays: r.clone() });
            }
            ada.room(RoomCommand::Lobby {
                invite: None,
                name: Some("Ada".into()),
                pick: Some(pick.clone()),
                anyway: false,
            });
        },
        vec![(
            &ada,
            b(Box::new(|e, v| {
                matches!(e, Event::RoomJoined(_) | Event::RoomState(_))
                    && v.is_some_and(|v| v.link.is_some() && v.room_episode.is_some())
            })),
        )],
    );
    run.learn("Ada", began);
    let Some(first) = ada.link() else {
        println!("✕ no room: stopping");
        finish(&run, &[&ada, &ren, &mika, &kai], &nip11, &shown, report.as_deref());
        return;
    };

    for (m, others) in [(&ren, vec![&ada]), (&mika, vec![&ada, &ren])] {
        let name = m.name;
        let mut waits: Vec<(&Member, Done)> = Vec::new();
        for o in &others {
            waits.push((m, Box::new(sees(o.name))));
        }
        for o in &others {
            waits.push((o, Box::new(sees(name))));
        }
        let link = first.clone();
        let began = run.step(
            &format!("{name} joins by the invite"),
            || {
                m.room(RoomCommand::Lobby {
                    invite: Some(link),
                    name: Some(name.into()),
                    pick: None,
                    anyway: false,
                })
            },
            waits,
        );
        run.learn(name, began);
    }

    // A line from each.
    for (m, others) in [(&ada, [&ren, &mika]), (&ren, [&ada, &mika]), (&mika, [&ada, &ren])] {
        let text = format!("a line from {}", m.name);
        let waits =
            others.iter().map(|o| (*o, b(Box::new(said(m.name, text.clone()))))).collect();
        run.step(
            &format!("{} says a line", m.name),
            || m.room(RoomCommand::Say { text: text.clone() }),
            waits,
        );
    }

    // Play: the room waits for those loading (no player here: everyone is) until they say
    // Don't wait for me.
    run.step(
        "Ada plays (the wait shows)",
        || ada.room(RoomCommand::Play { play: true }),
        [&ada, &ren, &mika]
            .into_iter()
            .map(|m| {
                (
                    m,
                    b(Box::new(|_: &Event, v: Option<&RoomView>| {
                        v.is_some_and(|v| v.playing || !v.waiting.is_empty())
                    })),
                )
            })
            .collect(),
    );
    run.step(
        "Ren and Mika: Don't wait for me",
        || {
            ren.room(RoomCommand::DontWait);
            mika.room(RoomCommand::DontWait);
        },
        [&ada, &ren, &mika]
            .into_iter()
            .map(|m| {
                (m, b(Box::new(|_: &Event, v: Option<&RoomView>| v.is_some_and(|v| v.playing))))
            })
            .collect(),
    );
    if !ada.view.lock().unwrap().as_ref().is_some_and(|v| v.playing) {
        // Still waiting on someone: Carry on.
        run.step(
            "Ada carries on",
            || ada.room(RoomCommand::CarryOn),
            [&ada, &ren, &mika]
                .into_iter()
                .map(|m| {
                    (
                        m,
                        b(Box::new(|_: &Event, v: Option<&RoomView>| {
                            v.is_some_and(|v| v.playing)
                        })),
                    )
                })
                .collect(),
        );
    }
    run.step(
        "Ren pauses",
        || ren.room(RoomCommand::Play { play: false }),
        [&ada, &mika]
            .into_iter()
            .map(|m| {
                (
                    m,
                    b(Box::new(|_: &Event, v: Option<&RoomView>| {
                        v.is_some_and(|v| !v.playing && v.paused_by.as_deref() == Some("Ren"))
                    })),
                )
            })
            .collect(),
    );
    run.step(
        "Mika skips the opening (a seek)",
        || mika.room(RoomCommand::SkipOpening { to: 90.0 }),
        [&ada, &ren]
            .into_iter()
            .map(|m| {
                (m, b(Box::new(|e: &Event, v: Option<&RoomView>| {
                    matches!(e, Event::RoomMessage(m) if matches!(&m.kind, RoomMessageKind::Seeked { by, .. } if by == "Mika"))
                        || v.is_some_and(|v| v.position.is_some_and(|p| (p - 90.0).abs() < 2.0))
                })))
            })
            .collect(),
    );

    // New invite, and a newcomer by it.
    let old = key_of(&first);
    run.step(
        "Ada makes a new invite",
        || ada.room(RoomCommand::NewInvite),
        vec![
            (&ada, b(Box::new(move |_: &Event, v: Option<&RoomView>| {
                v.is_some_and(|v| v.link.as_deref().is_some_and(|l| key_of(l) != old))
            }))),
            (&ren, b(Box::new(|e: &Event, _: Option<&RoomView>| matches!(e, Event::RoomMessage(m) if matches!(m.kind, RoomMessageKind::NewInvite { .. }))))),
            (&mika, b(Box::new(|e: &Event, _: Option<&RoomView>| matches!(e, Event::RoomMessage(m) if matches!(m.kind, RoomMessageKind::NewInvite { .. }))))),
        ],
    );
    let second = ada.link().unwrap_or_default();
    let began = run.step(
        "Kai joins by the new invite",
        || {
            kai.room(RoomCommand::Lobby {
                invite: Some(second),
                name: Some("Kai".into()),
                pick: None,
                anyway: false,
            })
        },
        vec![
            (&kai, b(Box::new(sees("Ada")))),
            (&kai, b(Box::new(sees("Ren")))),
            (&kai, b(Box::new(sees("Mika")))),
            (&ada, b(Box::new(sees("Kai")))),
            (&ren, b(Box::new(sees("Kai")))),
            (&mika, b(Box::new(sees("Kai")))),
        ],
    );
    run.learn("Kai", began);
    if linger > 0 {
        let marks: Vec<usize> = [&ada, &ren, &mika, &kai].iter().map(|m| m.mark()).collect();
        let began = Instant::now();
        thread::sleep(Duration::from_secs(linger));
        // Every relay list any member was told of meanwhile.
        for (m, from) in [&ada, &ren, &mika, &kai].iter().zip(marks) {
            let mut lists: Vec<String> = Vec::new();
            for (at, e) in &m.events.lock().unwrap()[from..] {
                if let Event::RoomState(v) = e {
                    let line: Vec<String> = v
                        .relays
                        .iter()
                        .map(|r| {
                            format!(
                                "{}{}",
                                short(&r.url),
                                if r.healthy { "" } else { " (down)" }
                            )
                        })
                        .collect();
                    let line = line.join(", ");
                    if lists.last() != Some(&line) {
                        println!(
                            "  {:>5.1}s {} relays {line}",
                            at.saturating_duration_since(began).as_secs_f64(),
                            m.name
                        );
                        lists.push(line);
                    }
                }
            }
        }
        run.steps.push(Step { name: format!("idle {linger} s"), began, seen: Vec::new() });
    }

    // Ren in the background (the flush), back (its sockets open again), and says something.
    run.step(
        "Ren goes to the background",
        || {
            ren.engine
                .send(Command::AppPhase { phase: AppState::Background { playing: false } });
        },
        vec![(
            &ren,
            b(Box::new(|e: &Event, _: Option<&RoomView>| matches!(e, Event::RoomFlushed))),
        )],
    );
    thread::sleep(Duration::from_secs(3));
    let text = "back from the background".to_owned();
    run.step(
        "Ren comes back and says a line",
        || {
            ren.engine.send(Command::AppPhase { phase: AppState::Foreground });
            ren.room(RoomCommand::Say { text: text.clone() });
        },
        [&ada, &mika, &kai]
            .into_iter()
            .map(|o| (o, b(Box::new(said("Ren", text.clone())))))
            .collect(),
    );

    run.step(
        "Mika leaves",
        || mika.room(RoomCommand::Leave),
        [&ada, &ren, &kai].into_iter().map(|o| (o, b(Box::new(gone("Mika"))))).collect(),
    );
    run.step(
        "Ada leaves",
        || ada.room(RoomCommand::Leave),
        [&ren, &kai].into_iter().map(|o| (o, b(Box::new(gone("Ada"))))).collect(),
    );
    run.step(
        "Ren and Kai leave: the room closes",
        || {
            ren.room(RoomCommand::Leave);
            kai.room(RoomCommand::Leave);
        },
        [&ren, &kai]
            .into_iter()
            .map(|m| {
                (
                    m,
                    b(Box::new(|e: &Event, _: Option<&RoomView>| {
                        matches!(e, Event::RoomLeft { .. })
                    })),
                )
            })
            .collect(),
    );
    // What is still in flight lands, then the engines stop.
    thread::sleep(Duration::from_secs(3));
    finish(&run, &[&ada, &ren, &mika, &kai], &nip11, &shown, report.as_deref());
    for m in [&ada, &ren, &mika, &kai] {
        m.engine.shutdown();
    }
}

// ── NIP-11 ───────────────────────────────────────────────────────────────────────────────

fn nip11_all(relays: &[String], spares: &[String]) -> Vec<(String, String)> {
    let runtime = tokio::runtime::Runtime::new().expect("a runtime");
    let http = reqwest::Client::builder().use_rustls_tls().build().expect("http");
    let mut out = Vec::new();
    for url in relays.iter().chain(spares) {
        let Some(info) = kaeri_room::clock::nip11_url(url) else { continue };
        let text = runtime.block_on(async {
            let reply = http
                .get(info)
                .header("Accept", "application/nostr+json")
                .timeout(Duration::from_secs(8))
                .send()
                .await;
            match reply {
                Ok(r) => r.text().await.unwrap_or_default(),
                Err(e) => format!("(unreachable: {e})"),
            }
        });
        let summary = match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(v) => {
                let mut s = String::new();
                for k in ["software", "version"] {
                    if let Some(x) = v.get(k).and_then(|x| x.as_str()) {
                        let _ = write!(s, "{k} {x}; ");
                    }
                }
                if let Some(n) = v.get("supported_nips") {
                    let _ = write!(s, "nips {n}; ");
                }
                if let Some(l) = v.get("limitation") {
                    let _ = write!(s, "limitation {l}; ");
                }
                for k in ["retention", "fees", "posting_policy"] {
                    if let Some(x) = v.get(k) {
                        let _ = write!(s, "{k} {x}; ");
                    }
                }
                s
            }
            Err(_) => text.chars().take(120).collect(),
        };
        println!("── NIP-11 {url}: {summary}");
        out.push((url.clone(), summary));
    }
    out
}

// ── the report ───────────────────────────────────────────────────────────────────────────

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn stats(v: &mut [f64]) -> String {
    if v.is_empty() {
        return "–".into();
    }
    v.sort_by(f64::total_cmp);
    format!("median {:.0} ms, worst {:.0} ms (n {})", v[v.len() / 2], v[v.len() - 1], v.len())
}

fn finish(
    run: &Run,
    members: &[&Member],
    nip11: &[(String, String)],
    relays: &[String],
    report: Option<&str>,
) {
    let recs = run.log.0.lock().unwrap().clone();
    let who = |pk: &str| run.who.get(pk).copied().unwrap_or("?");
    let mut md = String::new();
    let _ = writeln!(md, "Relays: {}\n", relays.join(", "));
    for (url, s) in nip11 {
        let _ = writeln!(md, "- NIP-11 {url}: {s}");
    }

    // Each event: who sent it and when (its first send), over which relays.
    let mut sent: BTreeMap<String, Sent> = BTreeMap::new();
    for r in &recs {
        if r.msg == "send" {
            let e = sent.entry(r.get("id").to_owned()).or_insert((
                r.at,
                r.get("me").to_owned(),
                BTreeSet::new(),
            ));
            e.2.insert(r.get("relay").to_owned());
        }
    }
    // Heard: (id, member) → per relay, the first time; and how many copies.
    let mut heard: HashMap<(String, &str), BTreeMap<String, Instant>> = HashMap::new();
    let mut copies: HashMap<(String, &str), usize> = HashMap::new();
    for r in recs.iter().filter(|r| r.msg == "heard") {
        let key = (r.get("id").to_owned(), who(r.get("me")));
        heard.entry(key.clone()).or_default().entry(r.get("relay").to_owned()).or_insert(r.at);
        *copies.entry(key).or_default() += 1;
    }
    // A member is in the room from its first send to the last event it heard.
    let mut window: HashMap<&str, (Instant, Instant)> = HashMap::new();
    for r in &recs {
        let name = who(r.get("me"));
        if r.msg == "send" {
            window.entry(name).or_insert((r.at, r.at));
        }
        if r.msg == "heard"
            && let Some(w) = window.get_mut(name)
        {
            w.1 = w.1.max(r.at);
        }
    }
    let mut oks: HashMap<String, Vec<(String, bool, String)>> = HashMap::new();
    for r in recs.iter().filter(|r| r.msg == "ok") {
        oks.entry(r.get("id").to_owned()).or_default().push((
            r.get("relay").to_owned(),
            r.get("accepted") == "true",
            r.get("answer").to_owned(),
        ));
    }

    let _ = writeln!(md, "\n## Steps\n");
    let _ = writeln!(
        md,
        "| step | apps saw it | events | delivery to members (first copy) | per relay accepted | missing |"
    );
    let _ = writeln!(md, "|---|---|---|---|---|---|");
    let mut all = Vec::new();
    let mut per_relay: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut refusals: BTreeMap<String, usize> = BTreeMap::new();
    let names: Vec<&str> = members.iter().map(|m| m.name).collect();
    for (n, step) in run.steps.iter().enumerate() {
        let end = run.steps.get(n + 1).map_or(Instant::now(), |s| s.began);
        let ids: Vec<(&String, &Sent)> =
            sent.iter().filter(|(_, (at, _, _))| *at >= step.began && *at < end).collect();
        let mut times = Vec::new();
        let mut accepted: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        let mut missing = Vec::new();
        for (id, (at, from, _)) in &ids {
            let from = who(from);
            for (relay, ok, msg) in oks.get(*id).into_iter().flatten() {
                let e = accepted.entry(short(relay)).or_default();
                e.1 += 1;
                if *ok {
                    e.0 += 1;
                } else {
                    *refusals.entry(format!("{} {}", short(relay), msg)).or_default() += 1;
                }
            }
            for name in &names {
                if *name == from || !window.get(name).is_some_and(|(a, b)| a <= at && at <= b) {
                    continue;
                }
                match heard.get(&((*id).clone(), *name)) {
                    Some(per) => {
                        let first = per.values().min().expect("one");
                        let d = ms(first.saturating_duration_since(*at));
                        times.push(d);
                        all.push(d);
                        for (relay, t) in per {
                            per_relay
                                .entry(short(relay))
                                .or_default()
                                .push(ms(t.saturating_duration_since(*at)));
                        }
                    }
                    None => missing.push(format!("{from}→{name}")),
                }
            }
        }
        let apps: Vec<String> = step
            .seen
            .iter()
            .map(|(n, t)| {
                t.map_or(format!("{n} ✕"), |t| format!("{n} {:.2}s", t.as_secs_f64()))
            })
            .collect();
        let acc: Vec<String> =
            accepted.iter().map(|(r, (a, t))| format!("{r} {a}/{t}")).collect();
        let _ = writeln!(
            md,
            "| {} | {} | {} | {} | {} | {} |",
            step.name,
            apps.join(", "),
            ids.len(),
            stats(&mut times),
            acc.join(", "),
            if missing.is_empty() {
                "–".into()
            } else {
                format!(
                    "{} ({})",
                    missing.len(),
                    missing.iter().take(6).cloned().collect::<Vec<_>>().join(" ")
                )
            },
        );
    }
    let _ = writeln!(md, "\n## Totals\n");
    let _ = writeln!(
        md,
        "- events sent: {} (each to every relay up); run {:.0} s",
        sent.len(),
        run.t0.elapsed().as_secs_f64()
    );
    let _ = writeln!(md, "- delivery, first copy, all members: {}", stats(&mut all));
    for (relay, v) in &mut per_relay {
        let _ = writeln!(md, "- delivery over {relay}: {}", stats(v));
    }
    let redundant: usize = copies.values().map(|c| c.saturating_sub(1)).sum();
    let twice: usize = copies
        .iter()
        .map(|(k, c)| c.saturating_sub(heard.get(k).map_or(0, BTreeMap::len)))
        .sum();
    let _ = writeln!(
        md,
        "- copies: {redundant} beyond the first (one per further relay is expected and deduped); {twice} the same relay sent twice"
    );
    let _ = writeln!(md, "\n## What the relays answered (other than OK true)\n");
    for (k, n) in &refusals {
        let _ = writeln!(md, "- OK false ×{n}: {k}");
    }
    let mut other: BTreeMap<String, usize> = BTreeMap::new();
    for r in &recs {
        let line = match r.msg.as_str() {
            "notice" => format!("NOTICE {} {}", short(r.get("relay")), r.get("answer")),
            "closed" => format!("CLOSED {} {}", short(r.get("relay")), r.get("answer")),
            "auth asked" => format!("AUTH challenge {}", short(r.get("relay"))),
            "lost" => format!("socket lost {} {}", short(r.get("relay")), r.get("why")),
            "unreachable" => format!("unreachable {} {}", short(r.get("relay")), r.get("why")),
            m if m.contains("dropped") => {
                format!("dropped by {}: {}", who(r.get("me")), r.get("why"))
            }
            m if m.contains("unusable") => {
                format!("unusable {} {}", short(r.get("relay")), r.get("why"))
            }
            _ => continue,
        };
        *other.entry(line).or_default() += 1;
    }
    for (k, n) in &other {
        let _ = writeln!(md, "- ×{n} {k}");
    }
    let _ = writeln!(md, "\n## Clock\n");
    let mut any = false;
    for r in recs.iter().filter(|r| r.msg.contains("clock corrected")) {
        any = true;
        let _ = writeln!(md, "- {} corrected by {} ms", who(r.get("me")), r.get("offset_ms"));
    }
    if !any {
        let _ = writeln!(
            md,
            "- no member corrected its clock (offset under the threshold, or no Date header)"
        );
    }
    println!("\n{md}");
    if let Some(path) = report {
        std::fs::write(path, &md).expect("the report");
        // The whole wire log beside it, ms from the start, members by name.
        let mut trace = String::new();
        for r in &recs {
            let mut f = r.f.clone();
            if let Some(me) = f.remove("me") {
                f.insert("me".into(), who(&me).to_owned());
            }
            let fields: Vec<String> = f.iter().map(|(k, v)| format!("{k}={v}")).collect();
            let _ = writeln!(
                trace,
                "{:>8} {} {}",
                r.at.saturating_duration_since(run.t0).as_millis(),
                r.msg,
                fields.join(" ")
            );
        }
        for m in members {
            for (at, e) in m.events.lock().unwrap().iter() {
                let mut text = format!("{e:?}");
                text.truncate(160);
                let _ = writeln!(
                    trace,
                    "{:>8} APP {} {}",
                    at.saturating_duration_since(run.t0).as_millis(),
                    m.name,
                    text
                );
            }
        }
        for s in &run.steps {
            let _ = writeln!(
                trace,
                "{:>8} STEP {}",
                s.began.saturating_duration_since(run.t0).as_millis(),
                s.name
            );
        }
        std::fs::write(format!("{path}.trace"), trace).expect("the trace");
    }
}

fn short(url: &str) -> String {
    url.trim_start_matches("wss://")
        .trim_start_matches("ws://")
        .trim_end_matches('/')
        .to_owned()
}
