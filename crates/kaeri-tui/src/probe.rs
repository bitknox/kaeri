//! What the terminal is asked at start-up, and the keys typed while it answers.
//!
//! The palette wants the terminal's background (OSC 11) and the image engine wants its
//! graphics support and cell size. Both are asked together, in one write ending on a status
//! report that every terminal answers, and read back through one reader under one short
//! deadline. Whatever arrives that is not an answer was typed: it is kept and handed to the
//! app as keys once its loop starts, rather than dropped the way a crate's own reader drops
//! it. A terminal that answers nothing costs the deadline once, not a timeout per query.
//!
//! The reader is Unix only. Elsewhere [`ask`] answers with what the crates find on their own
//! and keeps no keys.

use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::theme::Rgb;

/// How long the terminal has to answer every query. A terminal that answers at all does so
/// within milliseconds, ending on the status report; only one that answers nothing waits this
/// long.
pub const DEADLINE: Duration = Duration::from_millis(500);

/// What to ask.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Ask {
    /// The background colour, for the adaptive palette.
    pub background: bool,
    /// Graphics support and cell size, for the image engine.
    pub graphics: bool,
}

/// What the terminal said about graphics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GraphicsAnswer {
    /// The Kitty graphics protocol answered OK.
    pub kitty: bool,
    /// Its device attributes name Sixel.
    pub sixel: bool,
    /// The cell size in pixels, as width and height.
    pub cell: Option<(u16, u16)>,
}

/// What came back.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Answers {
    pub background: Option<Rgb>,
    /// `None` when graphics were not asked or the terminal did not answer in time.
    pub graphics: Option<GraphicsAnswer>,
    /// Whether the terminal answered the status report that ends the queries.
    pub answered: bool,
    /// The bytes typed while the terminal was asked, in order.
    pub typed: Vec<u8>,
}

impl Answers {
    /// The keys typed while the terminal was asked, as the app's loop takes them.
    pub fn typed_events(&self) -> Vec<Event> {
        keys_of(&self.typed)
    }
}

/// Ask the terminal what `ask` names. Nothing asked, nothing is written or read.
pub fn ask(ask: Ask) -> Answers {
    if ask == Ask::default() {
        return Answers::default();
    }
    #[cfg(unix)]
    {
        unix::ask(ask)
    }
    #[cfg(not(unix))]
    {
        Answers {
            background: ask.background.then(crate::theme::detect::detect_background).flatten(),
            ..Answers::default()
        }
    }
}

/// The query for `ask`: graphics as `ratatui-image` asks them (wrapped for tmux), the
/// background, and the status report last.
pub fn query(ask: Ask, tmux: bool, no_graphics_protocols: bool) -> String {
    use ratatui_image::picker::{
        ProtocolType,
        cap_parser::{Parser, QueryStdioOptions},
    };
    if ask.graphics {
        let mut options = QueryStdioOptions {
            terminal_background_color_osc: ask.background,
            ..QueryStdioOptions::default()
        };
        if no_graphics_protocols {
            options.blacklist_protocols = vec![ProtocolType::Kitty, ProtocolType::Sixel];
        }
        Parser::query(tmux, options)
    } else {
        "\x1b]11;?\x07\x1b[5n".to_owned()
    }
}

/// Splits what the terminal sends into answers to queries and bytes that were typed.
#[derive(Debug, Default)]
pub struct Split {
    /// An escape sequence begun and not yet ended.
    open: Vec<u8>,
    typed: Vec<u8>,
    answers: Vec<Vec<u8>>,
}

/// What an escape sequence begun so far turns out to be.
enum Seq {
    /// Not ended yet.
    Open,
    /// An answer to a query, whole.
    Answer,
    /// Something typed, such as an arrow key, whole.
    Typed,
}

impl Split {
    pub fn push(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if self.open.is_empty() {
                if byte == 0x1b {
                    self.open.push(byte);
                } else {
                    self.typed.push(byte);
                }
                continue;
            }
            if self.open.len() == 1 && !matches!(byte, b'[' | b']' | b'_' | b'P') {
                // Escape and anything else is a key: Esc, Alt with a key, or SS3. The byte
                // after it is looked at again on its own.
                self.typed.push(0x1b);
                self.open.clear();
                if byte == 0x1b {
                    self.open.push(byte);
                } else {
                    self.typed.push(byte);
                }
                continue;
            }
            if byte == 0x1b && self.open[1] == b'[' {
                // A control sequence cut short by another escape was typed.
                self.typed.append(&mut self.open);
                self.open.push(byte);
                continue;
            }
            self.open.push(byte);
            match classify(&self.open) {
                Seq::Open => {}
                Seq::Answer => self.answers.push(std::mem::take(&mut self.open)),
                Seq::Typed => self.typed.append(&mut self.open),
            }
        }
    }

    /// The answers whole so far, taken.
    pub fn take_answers(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.answers)
    }

    /// Everything typed, once nothing more will come. A control sequence left open is taken
    /// as typed; an unfinished string (an answer cut off) is dropped.
    pub fn finish(mut self) -> Vec<u8> {
        if matches!(self.open.get(1), None | Some(b'[')) {
            self.typed.append(&mut self.open);
        }
        self.typed
    }
}

fn classify(seq: &[u8]) -> Seq {
    match seq[1] {
        b'[' => {
            let Some(&last) = seq.last().filter(|_| seq.len() > 2) else { return Seq::Open };
            if !(0x40..=0x7e).contains(&last) {
                return Seq::Open;
            }
            let body = &seq[2..seq.len() - 1];
            let answer = match last {
                // Device attributes.
                b'c' => body.first() == Some(&b'?'),
                // The cell size.
                b't' => body.starts_with(b"6;"),
                // The status report.
                b'n' => body == b"0",
                // A cursor position report: nothing types one.
                b'R' => true,
                _ => false,
            };
            if answer { Seq::Answer } else { Seq::Typed }
        }
        // OSC, APC and DCS end on BEL (OSC only) or the string terminator.
        b']' if seq.last() == Some(&0x07) => Seq::Answer,
        _ if seq.len() > 3 && seq.ends_with(b"\x1b\\") => Seq::Answer,
        _ => Seq::Open,
    }
}

/// What the answers say, through `ratatui-image`'s parser of them. Whether the status report
/// was among them.
fn read_answers(
    answers: &[Vec<u8>],
    into: &mut Answers,
    graphics: &mut GraphicsAnswer,
) -> bool {
    use ratatui_image::picker::cap_parser::{Parser, Response};
    let mut status = false;
    for answer in answers {
        let mut parser = Parser::new();
        for &byte in answer {
            for response in parser.push(char::from(byte)) {
                match response {
                    Response::Kitty => graphics.kitty = true,
                    Response::Sixel => graphics.sixel = true,
                    Response::CellSize(cell) => graphics.cell = cell.or(graphics.cell),
                    Response::Background(r, g, b) => into.background = Some(Rgb::new(r, g, b)),
                    Response::Status => status = true,
                    _ => {}
                }
            }
        }
    }
    status
}

#[cfg(unix)]
mod unix {
    use std::{
        io::{Read, Write},
        os::fd::AsFd,
        time::Instant,
    };

    use rustix::{
        event::{PollFd, PollFlags, Timespec, poll},
        termios::{self, LocalModes, OptionalActions, SpecialCodeIndex},
    };

    use super::*;

    pub(super) fn ask(ask: Ask) -> Answers {
        let tty = std::fs::OpenOptions::new().read(true).write(true).open("/dev/tty"); // i18n-skip
        let Ok(mut tty) = tty else {
            return Answers::default();
        };
        let (tmux, no_protocols) = environment();
        ask_on(&mut tty, ask, tmux, no_protocols, DEADLINE)
    }

    /// Whether this runs in tmux, and whether the terminal's Kitty and Sixel are not to be
    /// used (WezTerm and Konsole), as `ratatui-image` decides both.
    fn environment() -> (bool, bool) {
        let var = |key: &str| std::env::var(key).ok().filter(|v| !v.is_empty());
        let tmux = var("TERM").is_some_and(|t| t.starts_with("tmux"))
            || var("TERM_PROGRAM").is_some_and(|t| t == "tmux");
        let no_protocols =
            var("WEZTERM_EXECUTABLE").is_some() || var("KONSOLE_VERSION").is_some();
        (tmux, no_protocols)
    }

    /// [`ask`] on `tty`, waiting for answers no longer than `deadline`.
    pub fn ask_on<T: Read + Write + AsFd>(
        tty: &mut T,
        ask: Ask,
        tmux: bool,
        no_protocols: bool,
        deadline: Duration,
    ) -> Answers {
        let Ok(before) = termios::tcgetattr(tty.as_fd()) else { return Answers::default() };
        let mut quiet = before.clone();
        // Bytes as they come, without echo; signals and the rest stay as they are.
        quiet.local_modes &= !(LocalModes::ICANON | LocalModes::ECHO);
        // A read answers with whatever has come, however little.
        quiet.special_codes[SpecialCodeIndex::VMIN] = 1;
        quiet.special_codes[SpecialCodeIndex::VTIME] = 0;
        if termios::tcsetattr(tty.as_fd(), OptionalActions::Now, &quiet).is_err() {
            return Answers::default();
        }
        let mut answers = Answers::default();
        let mut graphics = GraphicsAnswer::default();
        let mut split = Split::default();
        let query = query(ask, tmux, no_protocols);
        if tty.write_all(query.as_bytes()).and_then(|()| tty.flush()).is_ok() {
            let until = Instant::now() + deadline;
            let mut buf = [0u8; 1024];
            loop {
                let left = until.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    break;
                }
                let timeout = Timespec {
                    tv_sec: left.as_secs().try_into().unwrap_or(i64::MAX),
                    tv_nsec: left.subsec_nanos().into(),
                };
                let mut fds = [PollFd::new(&*tty, PollFlags::IN)];
                match poll(&mut fds, Some(&timeout)) {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(rustix::io::Errno::INTR) => continue,
                    Err(_) => break,
                }
                let n = match tty.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                split.push(&buf[..n]);
                if read_answers(&split.take_answers(), &mut answers, &mut graphics) {
                    answers.answered = true;
                    break;
                }
            }
        }
        let _ = termios::tcsetattr(tty.as_fd(), OptionalActions::Now, &before);
        if ask.graphics && answers.answered {
            answers.graphics = Some(graphics);
        }
        answers.typed = split.finish();
        tracing::debug!(
            answered = answers.answered,
            background = answers.background.is_some(),
            graphics = ?answers.graphics,
            typed = answers.typed.len(),
            "the terminal was asked"
        );
        answers
    }
}

#[cfg(unix)]
pub use unix::ask_on;

/// The keys in bytes typed at a terminal, as crossterm reads them in raw mode: characters,
/// Enter, Tab, Backspace, Esc, control and Alt keys, arrows, the editing keys and F1 to F12.
/// Anything else is left out.
pub fn keys_of(bytes: &[u8]) -> Vec<Event> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let (key, used) = key_at(&bytes[at..]);
        if let Some(key) = key {
            out.push(Event::Key(key));
        }
        at += used.max(1);
    }
    out
}

fn key(code: KeyCode, modifiers: KeyModifiers) -> Option<KeyEvent> {
    Some(KeyEvent::new(code, modifiers))
}

/// The key at the start of `bytes`, and how many bytes it took.
fn key_at(bytes: &[u8]) -> (Option<KeyEvent>, usize) {
    let none = KeyModifiers::NONE;
    match bytes[0] {
        b'\r' | b'\n' => (key(KeyCode::Enter, none), 1),
        b'\t' => (key(KeyCode::Tab, none), 1),
        0x7f | 0x08 => (key(KeyCode::Backspace, none), 1),
        0x00 => (key(KeyCode::Char(' '), KeyModifiers::CONTROL), 1),
        c @ 0x01..=0x1a => {
            (key(KeyCode::Char(char::from(c - 1 + b'a')), KeyModifiers::CONTROL), 1)
        }
        0x1b => escape(bytes),
        0x1c..=0x1f => (None, 1),
        _ => match utf8_char(bytes) {
            Some((c, used)) => (key(KeyCode::Char(c), shift_of(c)), used),
            None => (None, 1),
        },
    }
}

fn shift_of(c: char) -> KeyModifiers {
    if c.is_uppercase() { KeyModifiers::SHIFT } else { KeyModifiers::NONE }
}

fn utf8_char(bytes: &[u8]) -> Option<(char, usize)> {
    let len = match bytes[0] {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf7 => 4,
        _ => return None,
    };
    let text = std::str::from_utf8(bytes.get(..len)?).ok()?;
    text.chars().next().map(|c| (c, len))
}

fn escape(bytes: &[u8]) -> (Option<KeyEvent>, usize) {
    match bytes.get(1) {
        None | Some(0x1b) => (key(KeyCode::Esc, KeyModifiers::NONE), 1),
        Some(b'[') => csi(bytes),
        Some(b'O') => match bytes.get(2) {
            Some(&c) => (ss3(c).and_then(|code| key(code, KeyModifiers::NONE)), 3),
            None => (key(KeyCode::Char('O'), KeyModifiers::ALT), 2),
        },
        Some(_) => {
            let (inner, used) = key_at(&bytes[1..]);
            (inner.map(|k| KeyEvent::new(k.code, k.modifiers | KeyModifiers::ALT)), used + 1)
        }
    }
}

fn ss3(c: u8) -> Option<KeyCode> {
    Some(match c {
        b'A' => KeyCode::Up,
        b'B' => KeyCode::Down,
        b'C' => KeyCode::Right,
        b'D' => KeyCode::Left,
        b'H' => KeyCode::Home,
        b'F' => KeyCode::End,
        b'P'..=b'S' => KeyCode::F(c - b'P' + 1),
        _ => return None,
    })
}

fn csi(bytes: &[u8]) -> (Option<KeyEvent>, usize) {
    let Some(end) = bytes.iter().skip(2).position(|b| (0x40..=0x7e).contains(b)) else {
        return (None, bytes.len());
    };
    let end = end + 2;
    let used = end + 1;
    let params: Vec<u8> = std::str::from_utf8(&bytes[2..end])
        .unwrap_or("")
        .split(';')
        .map(|p| p.parse().unwrap_or(0))
        .collect();
    // xterm's modifier parameter: one more than the bits for Shift, Alt and Control.
    let modifiers = params.get(1).map_or(KeyModifiers::NONE, |&m| {
        let bits = m.saturating_sub(1);
        let mut out = KeyModifiers::NONE;
        if bits & 1 != 0 {
            out |= KeyModifiers::SHIFT;
        }
        if bits & 2 != 0 {
            out |= KeyModifiers::ALT;
        }
        if bits & 4 != 0 {
            out |= KeyModifiers::CONTROL;
        }
        out
    });
    let code = match bytes[end] {
        b'Z' => return (key(KeyCode::BackTab, KeyModifiers::SHIFT), used),
        b'~' => match params.first().copied().unwrap_or(0) {
            1 | 7 => KeyCode::Home,
            2 => KeyCode::Insert,
            3 => KeyCode::Delete,
            4 | 8 => KeyCode::End,
            5 => KeyCode::PageUp,
            6 => KeyCode::PageDown,
            n @ 11..=15 => KeyCode::F(n - 10),
            n @ 17..=21 => KeyCode::F(n - 11),
            n @ 23..=24 => KeyCode::F(n - 12),
            _ => return (None, used),
        },
        c => match ss3(c) {
            Some(code) => code,
            None => return (None, used),
        },
    };
    (key(code, modifiers), used)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(chunks: &[&[u8]]) -> (Vec<Vec<u8>>, Vec<u8>) {
        let mut split = Split::default();
        let mut answers = Vec::new();
        for chunk in chunks {
            split.push(chunk);
            answers.extend(split.take_answers());
        }
        (answers, split.finish())
    }

    #[test]
    fn answers_are_taken_out_and_keys_around_them_kept() {
        let (answers, typed) = split(&[
            b"j",
            b"\x1b]11;rgb:1e1e/1e1e/2e2e\x1b\\k\x1b[A",
            b"\x1b_Gi=31;OK\x1b\\\x1b[?64;4c",
            b"\x1b[6;16;8t\x1b[",
            b"0nq",
        ]);
        assert_eq!(answers.len(), 5);
        assert_eq!(typed, b"jk\x1b[Aq");
    }

    #[test]
    fn a_lone_escape_and_alt_keys_are_typed() {
        let (answers, typed) = split(&[b"\x1b", b"x\x1b\x1b", b"[B\x1bOP"]);
        assert!(answers.is_empty());
        assert_eq!(typed, b"\x1bx\x1b\x1b[B\x1bOP");
        // An answer cut off at the deadline is not taken for keys.
        let (_, typed) = split(&[b"a\x1b]11;rgb:1e1e"]);
        assert_eq!(typed, b"a");
    }

    #[test]
    fn what_the_answers_say_is_read() {
        let (answers, _) = split(&[
            b"\x1b]11;rgb:ffff/ffff/ffff\x07\x1b_Gi=31;OK\x1b\\\x1b[?62;4c\x1b[6;20;10t\x1b[0n",
        ]);
        let mut said = Answers::default();
        let mut graphics = GraphicsAnswer::default();
        assert!(read_answers(&answers, &mut said, &mut graphics));
        assert_eq!(said.background, Some(Rgb::new(255, 255, 255)));
        assert_eq!(graphics, GraphicsAnswer { kitty: true, sixel: true, cell: Some((10, 20)) });
    }

    #[test]
    fn typed_bytes_become_the_keys_crossterm_would_read() {
        let keys: Vec<KeyEvent> =
            keys_of(b"jK\n\t\x7f\x03\x1b[A\x1b[1;5C\x1b[5~\x1bOP\x1bx\xc3\xa9\x1b")
                .into_iter()
                .map(|e| match e {
                    Event::Key(k) => k,
                    other => panic!("{other:?}"),
                })
                .collect();
        let want = [
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('K'), KeyModifiers::SHIFT),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL),
            KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT),
            KeyEvent::new(KeyCode::Char('é'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        ];
        assert_eq!(keys, want);
    }

    #[test]
    fn nothing_asked_reads_nothing() {
        assert_eq!(ask(Ask::default()), Answers::default());
    }
}
