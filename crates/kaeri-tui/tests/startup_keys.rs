//! Keys typed while the terminal is asked about itself at start-up reach the app, whether the
//! terminal answers or not, and a terminal that answers nothing costs one short deadline.
//! A pseudo-terminal stands in for the terminal: its far side answers the queries, or stays
//! quiet, while keys are typed into it from 20 to 300 ms after the queries go out.
#![cfg(unix)]

use std::{
    fs::File,
    io::{Read, Write},
    os::fd::AsFd,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use kaeri_tui::{
    probe::{Ask, DEADLINE, ask_on, keys_of},
    theme::Rgb,
};
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt},
    termios::{self, LocalModes, OptionalActions},
};

/// The near side, as the app holds its terminal, and the far side, as the terminal.
fn pty() -> (File, File) {
    let far = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).unwrap();
    grantpt(&far).unwrap();
    unlockpt(&far).unwrap();
    let name = ptsname(&far, Vec::new()).unwrap();
    let near = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(name.to_str().unwrap())
        .unwrap();
    (near, File::from(far))
}

const ANSWERS: &[u8] =
    b"\x1b]11;rgb:ffff/ffff/ffff\x07\x1b_Gi=31;OK\x1b\\\x1b[?62;4c\x1b[6;20;10t\x1b[0n";

/// The keys typed, each with when, in milliseconds after the queries go out.
const TYPED: &[(u64, &[u8])] = &[(20, b"j"), (60, b"\x1b[B"), (120, b"k"), (300, b"W")];

fn wanted() -> Vec<Event> {
    [
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('W'), KeyModifiers::SHIFT),
    ]
    .into_iter()
    .map(Event::Key)
    .collect()
}

/// The terminal: once the queries have come, it types the keys on time and, when it answers,
/// answers after `answer_after`. It keeps reading what is written to it until told to stop.
fn terminal(
    far: File,
    answer_after: Option<Duration>,
    stop: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = far.try_clone().unwrap();
        let mut writer = far;
        let mut seen = Vec::new();
        let mut buf = [0u8; 1024];
        // The queries, which end on the status report.
        while !seen.windows(4).any(|w| w == b"\x1b[5n") {
            let n = reader.read(&mut buf).unwrap();
            seen.extend_from_slice(&buf[..n]);
        }
        let start = Instant::now();
        let typist = {
            let mut writer = writer.try_clone().unwrap();
            thread::spawn(move || {
                for (at, bytes) in TYPED {
                    let due = start + Duration::from_millis(*at);
                    thread::sleep(due.saturating_duration_since(Instant::now()));
                    writer.write_all(bytes).unwrap();
                }
            })
        };
        if let Some(after) = answer_after {
            thread::sleep(after);
            writer.write_all(ANSWERS).unwrap();
        }
        typist.join().unwrap();
        // Echo of keys typed after the queries ended is read and let go.
        while !stop.load(Ordering::SeqCst) {
            let mut fds = [PollFd::new(&reader, PollFlags::IN)];
            let wait = Timespec { tv_sec: 0, tv_nsec: 20_000_000 };
            if poll(&mut fds, Some(&wait)).unwrap_or(0) > 0 && reader.read(&mut buf).is_err() {
                break;
            }
        }
    })
}

/// What the app's own reader finds once it starts: whatever is waiting, read raw.
fn waiting(near: &mut File) -> Vec<u8> {
    let mut raw = termios::tcgetattr(near.as_fd()).unwrap();
    raw.local_modes &= !(LocalModes::ICANON | LocalModes::ECHO);
    termios::tcsetattr(near.as_fd(), OptionalActions::Now, &raw).unwrap();
    let mut out = Vec::new();
    let mut buf = [0u8; 256];
    loop {
        let mut fds = [PollFd::new(&*near, PollFlags::IN)];
        let wait = Timespec { tv_sec: 0, tv_nsec: 100_000_000 };
        if poll(&mut fds, Some(&wait)).unwrap() == 0 {
            return out;
        }
        let n = near.read(&mut buf).unwrap();
        out.extend_from_slice(&buf[..n]);
    }
}

fn run(answer_after: Option<Duration>) -> (kaeri_tui::probe::Answers, Duration, Vec<Event>) {
    let (mut near, far) = pty();
    let stop = Arc::new(AtomicBool::new(false));
    let terminal = terminal(far, answer_after, stop.clone());
    let started = Instant::now();
    let answers =
        ask_on(&mut near, Ask { background: true, graphics: true }, false, false, DEADLINE);
    let took = started.elapsed();
    // Past the last key, as the app's loop starts after the engine.
    thread::sleep(Duration::from_millis(400).saturating_sub(started.elapsed()));
    let mut bytes = answers.typed.clone();
    bytes.extend(waiting(&mut near));
    stop.store(true, Ordering::SeqCst);
    terminal.join().unwrap();
    (answers, took, keys_of(&bytes))
}

#[test]
fn keys_typed_while_a_terminal_answers_reach_the_app() {
    let (answers, took, keys) = run(Some(Duration::from_millis(90)));
    assert!(answers.answered);
    assert_eq!(answers.background, Some(Rgb::new(255, 255, 255)));
    let graphics = answers.graphics.unwrap();
    assert!(graphics.kitty && graphics.sixel);
    assert_eq!(graphics.cell, Some((10, 20)));
    assert!(took < DEADLINE, "the answers end the wait: {took:?}");
    // Keys typed before the answers were kept by the reader; the rest waited for the app.
    assert!(!answers.typed.is_empty(), "{:?}", answers.typed);
    assert_eq!(keys, wanted());
}

#[test]
fn keys_typed_at_a_terminal_that_answers_nothing_reach_the_app() {
    let (answers, took, keys) = run(None);
    assert!(!answers.answered);
    assert_eq!((answers.background, answers.graphics), (None, None));
    assert!(took >= DEADLINE && took < DEADLINE * 2, "one deadline for every query: {took:?}");
    assert_eq!(keys, wanted());
}
