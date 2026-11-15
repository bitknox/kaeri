//! The player log: what the player did in the last few playbacks, for Report a bug to save as a
//! file the reporter attaches to the issue by hand. Nothing here is ever sent.
//!
//! A tracing layer ([`layer`]) keeps, at info and above, the lines of the playback, resolve, proxy
//! and player targets, mpv's own (target `mpv`), and the app's lines under source `player` or
//! `mpv`: a hosted player sends mpv's info lines through `kaeri_ffi::log::log_line` while a
//! playback runs. Lines are grouped by playback: [`begin`] starts a group as each playback is
//! staged, and a line before the first is let go. It keeps the last [`PLAYBACKS`] playbacks, at
//! most [`CAP_BYTES`] in all. The oldest playback goes first; a single playback over the cap drops
//! its oldest lines and counts them.
//!
//! Every address is cut to its scheme and host as a line comes in, as the app log cuts mpv's
//! ([`kaeri_model::bug_report::cut_addresses`]). The log is mirrored to one capped file,
//! [`FILE`] in the cache's `logs` folder ([`mirror_to`]), so a crash in the player does not lose
//! it. The file is read back the first time the log is written or asked for after a start, not at
//! launch, and deleted once it is [`KEPT_DAYS`] old, like a crash report.
//!
//! Full redaction happens only when the text is asked for ([`text`]): the report's private strings
//! and sources, the playbacks' titles, and what the engine has seen to be private ([`note`]):
//! tracker account names, synced device names, party rooms, store links, server addresses and
//! search words.

use std::{
    collections::{BTreeSet, VecDeque},
    fmt,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError},
};

use jiff::{Timestamp, tz::TimeZone};
use kaeri_model::{
    bug_report::{BugReport, LoggedLine, LoggedPlayback, PlayerLogStats, cut_addresses},
    event::Event,
};
use tracing::{
    Level, Metadata, Subscriber,
    field::{Field, Visit},
};
use tracing_subscriber::{Layer, filter::filter_fn, layer::Context, registry::LookupSpan};

pub use kaeri_model::crash::KEPT_DAYS;

/// How many playbacks the log keeps.
pub const PLAYBACKS: usize = 3;
/// The most the kept lines may come to, in bytes.
pub const CAP_BYTES: usize = 1_000_000;
/// The mirror's file name, in the cache's `logs` folder.
pub const FILE: &str = "player-log.txt";
/// The mirror is written over from what is kept once it grows past this.
const FILE_CAP: u64 = 2 * CAP_BYTES as u64;
/// The longest line kept, in characters.
const LINE_CHARS: usize = 600;
/// The most private strings remembered from the engine's events.
const NAMES: usize = 512;
/// The mirror's first line, which says how the rest is written.
const HEADER: &str = "kaeri player log 1";
const DAY_MS: i64 = 86_400_000;

/// How much a line matters: info and above are kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Severity {
    Error,
    Warn,
    Info,
}

impl Severity {
    fn word(self) -> &'static str {
        match self {
            Self::Error => "ERROR",
            Self::Warn => "WARN",
            Self::Info => "INFO",
        }
    }

    fn letter(self) -> char {
        match self {
            Self::Error => 'E',
            Self::Warn => 'W',
            Self::Info => 'I',
        }
    }

    fn of_letter(letter: &str) -> Option<Self> {
        match letter {
            "E" => Some(Self::Error),
            "W" => Some(Self::Warn),
            "I" => Some(Self::Info),
            _ => None,
        }
    }

    fn of_level(level: &Level) -> Option<Self> {
        match *level {
            Level::ERROR => Some(Self::Error),
            Level::WARN => Some(Self::Warn),
            Level::INFO => Some(Self::Info),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
struct Line {
    at_ms: i64,
    severity: Severity,
    source: String,
    text: String,
}

impl Line {
    /// What the line counts for against the cap: about what it takes in the file.
    fn bytes(&self) -> usize {
        self.text.len() + self.source.len() + 24
    }
}

#[derive(Debug, Clone)]
struct Playback {
    started_ms: i64,
    episode: String,
    titles: Vec<String>,
    lines: VecDeque<Line>,
    bytes: usize,
    dropped: u64,
}

/// The kept playbacks, oldest first.
#[derive(Debug, Default)]
struct Ring {
    playbacks: VecDeque<Playback>,
    bytes: usize,
}

impl Ring {
    fn begin(&mut self, started_ms: i64, episode: String, titles: Vec<String>) {
        let bytes = episode.len() + titles.iter().map(String::len).sum::<usize>() + 24;
        self.playbacks.push_back(Playback {
            started_ms,
            episode,
            titles,
            lines: VecDeque::new(),
            bytes,
            dropped: 0,
        });
        self.bytes += bytes;
        while self.playbacks.len() > PLAYBACKS {
            self.drop_oldest();
        }
        self.trim();
    }

    /// Keep `line` in the playback under way; false, and nothing kept, before the first.
    fn push(&mut self, line: Line) -> bool {
        let Some(current) = self.playbacks.back_mut() else { return false };
        let bytes = line.bytes();
        current.lines.push_back(line);
        current.bytes += bytes;
        self.bytes += bytes;
        self.trim();
        true
    }

    fn drop_oldest(&mut self) {
        if let Some(gone) = self.playbacks.pop_front() {
            self.bytes -= gone.bytes;
        }
    }

    /// Under the cap again: the oldest playback goes first, then the oldest lines of the one
    /// left, counted.
    fn trim(&mut self) {
        while self.bytes > CAP_BYTES {
            if self.playbacks.len() > 1 {
                self.drop_oldest();
                continue;
            }
            let Some(only) = self.playbacks.back_mut() else { return };
            let Some(line) = only.lines.pop_front() else { return };
            only.bytes -= line.bytes();
            only.dropped += 1;
            self.bytes -= line.bytes();
        }
    }

    fn lines(&self) -> usize {
        self.playbacks.iter().map(|playback| playback.lines.len()).sum()
    }
}

/// One field of a record, on one line: no tab or line break in it.
fn flat(text: &str) -> String {
    text.chars().map(|c| if c == '\t' || c.is_control() { ' ' } else { c }).collect()
}

/// The records of the mirror that `ring` comes to.
fn records(ring: &Ring) -> String {
    let mut out = format!("{HEADER}\n");
    for playback in &ring.playbacks {
        out.push_str(&begin_record(playback.started_ms, &playback.episode, &playback.titles));
        if playback.dropped > 0 {
            out.push_str(&format!("D\t{}\n", playback.dropped));
        }
        for line in &playback.lines {
            out.push_str(&line_record(line));
        }
    }
    out
}

fn begin_record(started_ms: i64, episode: &str, titles: &[String]) -> String {
    let mut record = format!("P\t{started_ms}\t{}", flat(episode));
    for title in titles {
        record.push('\t');
        record.push_str(&flat(title));
    }
    record.push('\n');
    record
}

fn line_record(line: &Line) -> String {
    format!(
        "L\t{}\t{}\t{}\t{}\n",
        line.at_ms,
        line.severity.letter(),
        flat(&line.source),
        flat(&line.text)
    )
}

/// Replay a mirror's records into a ring, as they were kept. `None` for a file this does not
/// write, which is then left out.
fn replay(reader: impl BufRead) -> Option<(Ring, i64)> {
    let mut lines = reader.lines();
    if lines.next()?.ok()? != HEADER {
        return None;
    }
    let mut ring = Ring::default();
    let mut newest = i64::MIN;
    for record in lines {
        let Ok(record) = record else { break };
        let mut fields = record.split('\t');
        match fields.next() {
            Some("P") => {
                let Some(Ok(started_ms)) = fields.next().map(str::parse::<i64>) else {
                    continue;
                };
                let episode = fields.next().unwrap_or_default().to_owned();
                let titles = fields.map(str::to_owned).filter(|t| !t.is_empty()).collect();
                newest = newest.max(started_ms);
                ring.begin(started_ms, episode, titles);
            }
            Some("D") => {
                let dropped = fields.next().and_then(|n| n.parse::<u64>().ok()).unwrap_or(0);
                if let Some(current) = ring.playbacks.back_mut() {
                    current.dropped += dropped;
                }
            }
            Some("L") => {
                let (Some(Ok(at_ms)), Some(severity), Some(source)) = (
                    fields.next().map(str::parse::<i64>),
                    fields.next().and_then(Severity::of_letter),
                    fields.next(),
                ) else {
                    continue;
                };
                let text = fields.collect::<Vec<_>>().join(" ");
                newest = newest.max(at_ms);
                ring.push(Line { at_ms, severity, source: source.to_owned(), text });
            }
            _ => {}
        }
    }
    Some((ring, newest))
}

/// The mirror: where the file is, and the file once it is open for adding to.
#[derive(Debug)]
struct Mirror {
    path: PathBuf,
    /// Open once what was in the file has been read back and written over.
    file: Option<File>,
    /// How much the file holds, in bytes.
    written: u64,
    /// Whether the file has been read back since the start.
    loaded: bool,
    /// A write failed: the log carries on in memory only.
    broken: bool,
}

impl Mirror {
    /// Write the whole of `ring` over the file, and keep it open for adding to.
    fn rewrite(&mut self, ring: &Ring) {
        let all = records(ring);
        let written = std::fs::write(&self.path, all.as_bytes()).and_then(|()| {
            OpenOptions::new().append(true).open(&self.path).map(|file| (file, all.len()))
        });
        match written {
            Ok((file, len)) => {
                self.file = Some(file);
                self.written = len as u64;
            }
            Err(_) => self.broken = true,
        }
    }

    fn add(&mut self, record: &str, ring: &Ring) {
        if self.broken || !self.loaded {
            return;
        }
        if self.written + record.len() as u64 > FILE_CAP {
            // `ring` already holds the record.
            self.rewrite(ring);
            return;
        }
        let Some(file) = self.file.as_mut() else { return };
        if file.write_all(record.as_bytes()).is_err() {
            self.broken = true;
            return;
        }
        self.written += record.len() as u64;
    }
}

#[derive(Debug, Default)]
struct State {
    ring: Ring,
    mirror: Option<Mirror>,
    /// What the engine has seen to be private, from its events.
    names: BTreeSet<String>,
    /// The last stats worked out, and what they were worked out from: see [`PlayerLog::stats`].
    stats: Option<(u64, PlayerLogStats)>,
    /// Counts every change to what the stats are worked out from.
    changes: u64,
    /// A private playback is under way: nothing is kept until a playback that is not private
    /// begins. Never written to the mirror.
    held: bool,
}

impl State {
    /// Read the mirror back, once: what it held goes before what was kept since the start, and
    /// the file is written over with the whole. A file older than [`KEPT_DAYS`] is deleted.
    fn load(&mut self, now_ms: i64) {
        let Some(mirror) = self.mirror.as_mut().filter(|mirror| !mirror.loaded) else { return };
        mirror.loaded = true;
        let earlier = File::open(&mirror.path)
            .ok()
            .and_then(|file| replay(BufReader::new(file)))
            .filter(|(_, newest)| now_ms.saturating_sub(*newest) <= KEPT_DAYS * DAY_MS);
        if let Some((mut ring, _)) = earlier {
            for playback in std::mem::take(&mut self.ring.playbacks) {
                ring.begin(playback.started_ms, playback.episode, playback.titles);
                for line in playback.lines {
                    ring.push(line);
                }
            }
            self.ring = ring;
            self.changes += 1;
        }
        mirror.rewrite(&self.ring);
    }
}

/// The log: the kept playbacks, their mirror, and the private strings seen. The one in use is
/// the process's ([`installed`]); a test makes its own.
#[derive(Debug, Default)]
pub struct PlayerLog {
    state: Mutex<State>,
}

impl PlayerLog {
    pub fn new() -> Self {
        Self::default()
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Mirror the log to [`FILE`] in `dir`, once: a second call changes nothing. A file there
    /// older than [`KEPT_DAYS`] by its last write is deleted now; the rest is read back the
    /// first time the log is written or asked for.
    pub fn mirror_to(&self, dir: &Path, now_ms: i64) {
        let mut state = self.state();
        if state.mirror.is_some() {
            return;
        }
        let path = dir.join(FILE);
        let modified = std::fs::metadata(&path).and_then(|meta| meta.modified()).ok();
        if let Some(modified) = modified {
            let modified_ms = modified
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| i64::try_from(since.as_millis()).unwrap_or(i64::MAX));
            if now_ms.saturating_sub(modified_ms) > KEPT_DAYS * DAY_MS {
                let _ = std::fs::remove_file(&path);
            }
        }
        let _ = std::fs::create_dir_all(dir);
        state.mirror =
            Some(Mirror { path, file: None, written: 0, loaded: false, broken: false });
    }

    /// A playback starts: its lines go under it from now on. `titles` are the show's title and
    /// the episode's own, which are removed wherever they appear when the text is asked for.
    pub fn begin(&self, episode: &str, titles: &[String], now_ms: i64) {
        let mut state = self.state();
        state.load(now_ms);
        let titles: Vec<String> =
            titles.iter().map(|t| flat(t.trim())).filter(|t| !t.is_empty()).collect();
        let episode = flat(episode.trim());
        let record = begin_record(now_ms, &episode, &titles);
        state.held = false;
        state.ring.begin(now_ms, episode, titles);
        state.changes += 1;
        let State { ring, mirror, .. } = &mut *state;
        if let Some(mirror) = mirror {
            mirror.add(&record, ring);
        }
    }

    /// A private playback starts: it is not kept, its titles go nowhere, and no line is kept
    /// until a playback that is not private begins.
    pub fn hold(&self) {
        self.state().held = true;
    }

    /// Keep one line, its addresses cut, under the playback under way. Nothing is kept before
    /// the first playback, nor while a private one plays.
    fn line(&self, at_ms: i64, severity: Severity, source: &str, text: &str) {
        let mut state = self.state();
        if state.held {
            return;
        }
        state.load(at_ms);
        let text: String = cut_addresses(text).chars().take(LINE_CHARS).collect();
        let line = Line { at_ms, severity, source: source.to_owned(), text };
        let record = line_record(&line);
        if !state.ring.push(line) {
            return;
        }
        state.changes += 1;
        let State { ring, mirror, .. } = &mut *state;
        if let Some(mirror) = mirror {
            mirror.add(&record, ring);
        }
    }

    /// Forget every playback, here and in the mirror.
    #[cfg(any(test, feature = "testing"))]
    fn clear(&self, now_ms: i64) {
        let mut state = self.state();
        state.load(now_ms);
        state.ring = Ring::default();
        state.changes += 1;
        let State { ring, mirror, .. } = &mut *state;
        if let Some(mirror) = mirror.as_mut().filter(|mirror| !mirror.broken) {
            mirror.rewrite(ring);
        }
    }

    /// Remember strings the engine has seen to be private, for the text to remove.
    pub fn remember(&self, private: impl IntoIterator<Item = String>) {
        let mut state = self.state();
        let before = state.names.len();
        for text in private {
            let text = text.trim();
            if text.chars().count() >= 2 && state.names.len() < NAMES {
                state.names.insert(text.to_owned());
            }
        }
        if state.names.len() != before {
            state.changes += 1;
        }
    }

    /// The kept playbacks, oldest first, with times in `zone`, and the private strings seen.
    fn snapshot(&self, zone: &TimeZone, now_ms: i64) -> (Vec<LoggedPlayback>, Vec<String>) {
        let mut state = self.state();
        state.load(now_ms);
        let playbacks = state
            .ring
            .playbacks
            .iter()
            .map(|playback| LoggedPlayback {
                episode: playback.episode.clone(),
                started: clock_time(playback.started_ms, zone, false),
                titles: playback.titles.clone(),
                lines: playback
                    .lines
                    .iter()
                    .map(|line| LoggedLine {
                        time: clock_time(line.at_ms, zone, true),
                        level: line.severity.word().to_owned(),
                        source: line.source.clone(),
                        message: line.text.clone(),
                    })
                    .collect(),
                dropped: playback.dropped,
            })
            .collect();
        (playbacks, state.names.iter().cloned().collect())
    }

    /// The redacted file for `report`: its summary, then each playback, newest first.
    pub fn text(&self, report: &BugReport, zone: &TimeZone, now_ms: i64) -> String {
        let (playbacks, names) = self.snapshot(zone, now_ms);
        kaeri_model::bug_report::player_log_text(report, &playbacks, &names)
    }

    /// How many playbacks the log holds, the size of its redacted file, and the file's name for
    /// `now_ms` in `zone`. The size is worked out again only once the log has changed.
    pub fn stats(&self, zone: &TimeZone, now_ms: i64) -> PlayerLogStats {
        let file_name = file_name(now_ms, zone);
        let (changes, kept) = {
            let mut state = self.state();
            state.load(now_ms);
            (state.changes, state.stats.clone())
        };
        if let Some((_, stats)) = kept.filter(|(at, _)| *at == changes) {
            return PlayerLogStats { file_name, ..stats };
        }
        let (playbacks, names) = self.snapshot(zone, now_ms);
        let bytes = if playbacks.is_empty() {
            0
        } else {
            kaeri_model::bug_report::player_log_text(&BugReport::default(), &playbacks, &names)
                .len() as u64
        };
        let stats = PlayerLogStats { playbacks: playbacks.len() as u32, bytes, file_name };
        self.state().stats = Some((changes, stats.clone()));
        stats
    }

    /// How many lines are kept, over every playback.
    pub fn lines(&self) -> usize {
        self.state().ring.lines()
    }
}

/// `14:21` or `14:21:07.412`, in `zone`.
fn clock_time(at_ms: i64, zone: &TimeZone, seconds: bool) -> String {
    let Ok(at) = Timestamp::from_millisecond(at_ms) else { return String::new() };
    let at = at.to_zoned(zone.clone());
    if seconds {
        format!(
            "{:02}:{:02}:{:02}.{:03}",
            at.hour(),
            at.minute(),
            at.second(),
            at.millisecond()
        )
    } else {
        format!("{:02}:{:02}", at.hour(), at.minute())
    }
}

/// The saved file's name for `now_ms`, in `zone`: `kaeri-player-log-2026-11-15-1432.txt`.
pub fn file_name(now_ms: i64, zone: &TimeZone) -> String {
    let at = Timestamp::from_millisecond(now_ms).unwrap_or_default().to_zoned(zone.clone());
    format!(
        "kaeri-player-log-{:04}-{:02}-{:02}-{:02}{:02}.txt",
        at.year(),
        at.month(),
        at.day(),
        at.hour(),
        at.minute()
    )
}

/// Now, in Unix milliseconds: the engine's stopped clock while scenes are recorded.
pub fn now_ms() -> i64 {
    crate::clock::stopped().map_or_else(crate::crash::now_ms, |at| at.saturating_mul(1000))
}

// The process's log, which the layer feeds and the front ends read.

static LOG: OnceLock<Arc<PlayerLog>> = OnceLock::new();

/// The process's log, once a layer is installed.
pub fn installed() -> Option<&'static Arc<PlayerLog>> {
    LOG.get()
}

/// The layer that keeps the player log, for a front end's subscriber, beside its own layers.
/// It filters for itself, so give the others their own filters (`with_filter`) rather than the
/// subscriber one for all: `KAERI_LOG` or a quieter default must not take the player's lines
/// from it.
pub fn layer<S>() -> impl Layer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let log = Arc::clone(LOG.get_or_init(Arc::default));
    Keeper { log }.with_filter(filter_fn(wanted))
}

/// Mirror the process's log to [`FILE`] in `dir`, the cache's `logs` folder: see
/// [`PlayerLog::mirror_to`]. Nothing until a layer is installed.
pub fn mirror_to(dir: &Path) {
    if let Some(log) = LOG.get() {
        log.mirror_to(dir, now_ms());
    }
}

/// A playback of `episode` of `title` starts, with the episode's own name when it has one. A
/// `private` one is held: see [`PlayerLog::hold`]. Its names are also left out of every log
/// from now on, before any line of it is written.
pub fn begin(episode: &str, title: &str, episode_title: Option<&str>, private: bool) {
    if private {
        crate::private::redact::word(title);
        if let Some(episode_title) = episode_title {
            crate::private::redact::word(episode_title);
        }
        if let Some(log) = LOG.get() {
            log.hold();
        }
        return;
    }
    if let Some(log) = LOG.get() {
        let titles: Vec<String> =
            std::iter::once(title).chain(episode_title).map(str::to_owned).collect();
        log.begin(episode, &titles, now_ms());
    }
}

/// The redacted file for `report`, in the device's time zone. Empty playbacks, but the
/// summary, before anything has played.
pub fn text(report: &BugReport) -> String {
    match LOG.get() {
        Some(log) => log.text(report, &TimeZone::system(), now_ms()),
        None => PlayerLog::new().text(report, &TimeZone::system(), now_ms()),
    }
}

/// What the Player log row says, and the saved file's name, in the device's time zone.
pub fn stats() -> PlayerLogStats {
    match LOG.get() {
        Some(log) => log.stats(&TimeZone::system(), now_ms()),
        None => PlayerLogStats {
            file_name: file_name(now_ms(), &TimeZone::system()),
            ..Default::default()
        },
    }
}

/// For the Fixtures apps: the process's log holds `playbacks` made-up playbacks of Stardust
/// Wayfarer, about 100 kB each, the newest ending in a playback error, and nothing else. Zero
/// leaves it empty, as before anything has played. Installs the log when no layer has.
#[cfg(any(test, feature = "testing"))]
pub fn seed_fixture(playbacks: u32) {
    let log = LOG.get_or_init(Arc::default);
    let now = now_ms();
    log.clear(now);
    fixture_into(log, playbacks, now);
}

#[cfg(any(test, feature = "testing"))]
fn fixture_into(log: &PlayerLog, playbacks: u32, now: i64) {
    const STATUS_LINES: i64 = 1_000;
    for n in 0..i64::from(playbacks) {
        let episode = (n + 1).to_string();
        // Two hours apart, the newest ending a few minutes ago.
        let start = now - (i64::from(playbacks) - n) * 7_200_000;
        log.begin(&episode, &["Stardust Wayfarer".to_owned()], start);
        let at = |second: i64| start + second * 1_000;
        let info = Severity::Info;
        log.line(at(0), info, "player", &format!("staging episode {episode}"));
        log.line(
            at(1),
            info,
            "resolve",
            "picked a 1080p release from fixtures.example, 3 found",
        );
        log.line(at(1), info, "proxy", "serving the stream at http://127.0.0.1:41234/stream");
        log.line(at(2), info, "mpv", "video: h264 1920x1080 23.976 fps");
        log.line(at(2), info, "mpv", "audio: aac 2ch 48000 Hz, Japanese");
        log.line(at(2), info, "mpv", "subtitles: ass, English");
        for second in 0..STATUS_LINES {
            let clock = format!("{:02}:{:02}", second / 60, second % 60);
            let cache = 20 + second % 11;
            log.line(
                at(3 + second),
                info,
                "mpv",
                &format!(
                    "AV: 00:{clock} / 00:23:40 ({}%) A-V: 0.000 Dropped: 0 Cache: {cache}s/{}MB                      speed 1.00x",
                    second * 100 / 1_420,
                    cache + 6,
                ),
            );
            if second % 250 == 249 {
                log.line(
                    at(3 + second),
                    Severity::Warn,
                    "mpv",
                    "cache ran dry, buffering 1.4s",
                );
            }
        }
        let end = at(4 + STATUS_LINES);
        if n + 1 == i64::from(playbacks) {
            log.line(end, Severity::Error, "player", "the stream stopped: connection reset");
        } else {
            log.line(end, info, "player", "played to the end");
        }
    }
}

/// Remember what `event` shows to be private: an account's name, a synced device's, a party's
/// room, a store's link, a server's address, a search's words. Only while a layer is installed.
pub fn note(event: &Event) {
    let Some(log) = LOG.get() else { return };
    let private = private_in(event);
    if !private.is_empty() {
        log.remember(private);
    }
}

fn private_in(event: &Event) -> Vec<String> {
    use kaeri_model::{servers::ServerEvent, stores::InvitePreview};
    let mut out: Vec<String> = Vec::new();
    match event {
        Event::Sync(state) => out.extend(state.user.clone()),
        Event::SyncStatus(status) => {
            out.extend(status.device_name.clone());
            out.extend(status.devices.iter().map(|device| device.name.clone()));
            if let Some(folder) = &status.folder {
                out.push(folder.label.clone());
                out.extend(folder.account.clone());
            }
            out.extend(status.clouds.iter().filter_map(|cloud| cloud.account.clone()));
        }
        Event::Stores(rows) => {
            for row in rows {
                out.extend([row.url.clone(), row.host.clone(), row.member.clone()]);
            }
        }
        Event::StorePreview(InvitePreview::Store(store)) => {
            out.extend([store.link.0.clone(), store.url.clone(), store.host.clone()]);
        }
        Event::InviteLink(view) => {
            out.push(view.link.0.clone());
            out.extend(view.for_whom.clone());
        }
        Event::OpenJoin { view, .. } => {
            out.extend(view.link.as_ref().map(|link| link.0.clone()))
        }
        Event::PartyJoined(view) | Event::PartyState(view) => {
            out.extend([view.room.clone(), view.server.clone(), view.name.clone()]);
            out.extend(view.invite.clone());
        }
        Event::PartyRecent(rooms) => {
            for room in rooms {
                out.extend([room.room.clone(), room.server.clone(), room.title.clone()]);
                out.extend(room.names.iter().cloned());
            }
        }
        Event::Server(ServerEvent::Servers(rows)) => {
            for row in rows {
                out.extend([row.address.clone(), row.user.clone(), row.name.clone()]);
            }
        }
        Event::Server(ServerEvent::Checked(server)) => {
            out.extend([server.address.clone(), server.name.clone()]);
        }
        Event::Server(ServerEvent::Found(server)) => {
            out.extend([server.address.clone(), server.name.clone()]);
        }
        Event::SearchPaging { query, .. } | Event::SearchMore { query, .. } => {
            out.push(query.text.clone());
        }
        Event::MatchSearchFailed { query, .. } => out.push(query.clone()),
        Event::OpenedChoices { link, query, .. } => out.extend([link.clone(), query.clone()]),
        _ => {}
    }
    // A search of a word or two letters would take them out of every line: the redaction's
    // rules for words take what is left.
    out.retain(|text| text.trim().chars().count() >= 3);
    out
}

/// Whether a line from `target` may be the player log's: see the module's docs.
fn wanted(meta: &Metadata<'_>) -> bool {
    if *meta.level() > Level::INFO {
        return false;
    }
    let target = meta.target();
    if matches!(target, "mpv" | "app") {
        return true;
    }
    let crate_of = |name: &str| {
        target.strip_prefix(name).is_some_and(|rest| rest.is_empty() || rest.starts_with("::"))
    };
    if crate_of("kaeri_player") || crate_of("kaeri_playback") || crate_of("kaeri_mpv_host") {
        return true;
    }
    target
        .split("::")
        .skip(1)
        .any(|part| matches!(part, "playback" | "resolve" | "proxy" | "player"))
}

/// The word a line from `target` is kept under: `resolve`, `proxy`, `player` or `mpv`.
fn source_of(target: &str) -> &'static str {
    if target == "mpv" {
        return "mpv";
    }
    let parts = || target.split("::").skip(1);
    if parts().any(|part| part == "proxy") {
        "proxy"
    } else if parts().any(|part| part == "resolve") {
        "resolve"
    } else {
        "player"
    }
}

/// Fields whose values name something private: left out as they come in.
const PRIVATE_FIELDS: &[&str] = &[
    "id",
    "title",
    "episode_title",
    "query",
    "url",
    "link",
    "path",
    "file",
    "user",
    "name",
    "room",
    "server",
];

/// What an event said, as one line: its message, then its other fields as `key=value`.
#[derive(Default)]
struct Said {
    message: String,
    fields: String,
    /// The app's `source` field, for a line from target `app`.
    source: Option<String>,
}

impl Said {
    fn put(&mut self, field: &Field, value: String) {
        match field.name() {
            "message" => self.message = value,
            "source" => self.source = Some(value),
            name => {
                if !self.fields.is_empty() {
                    self.fields.push(' ');
                }
                if PRIVATE_FIELDS.contains(&name) {
                    self.fields.push_str(&format!("{name}=[removed]"));
                } else {
                    self.fields.push_str(&format!("{name}={value}"));
                }
            }
        }
    }

    fn text(self) -> String {
        match (self.message.is_empty(), self.fields.is_empty()) {
            (_, true) => self.message,
            (true, false) => self.fields,
            (false, false) => format!("{} {}", self.message, self.fields),
        }
    }
}

impl Visit for Said {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.put(field, value.to_owned());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.put(field, format!("{value:?}"));
    }
}

/// The layer itself, over the process's log.
struct Keeper {
    log: Arc<PlayerLog>,
}

impl<S: Subscriber> Layer<S> for Keeper {
    fn on_event(&self, event: &tracing::Event<'_>, _: Context<'_, S>) {
        let meta = event.metadata();
        let Some(severity) = Severity::of_level(meta.level()) else { return };
        let mut said = Said::default();
        event.record(&mut said);
        let source = match meta.target() {
            "app" => match said.source.as_deref().map(str::trim) {
                Some("player") => "player",
                Some("mpv") => "mpv",
                _ => return,
            },
            target => source_of(target),
        };
        // Private mode's redaction first, as the file and stderr layers have it: nothing while a
        // private play or read is open, and the private words and ids learnt taken out of the
        // rest.
        if crate::private::redact::active() {
            return;
        }
        let text = crate::private::redact::line(&said.text());
        self.log.line(now_ms(), severity, source, &text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::layer::SubscriberExt;

    const T0: i64 = 1_794_752_520_000; // 2026-11-15 14:22 UTC

    fn utc() -> TimeZone {
        TimeZone::UTC
    }

    fn line(log: &PlayerLog, at: i64, text: &str) {
        log.line(at, Severity::Info, "player", text);
    }

    fn texts(log: &PlayerLog) -> Vec<LoggedPlayback> {
        log.snapshot(&utc(), T0).0
    }

    #[test]
    fn nothing_is_kept_before_the_first_playback() {
        let log = PlayerLog::new();
        line(&log, T0, "before");
        assert_eq!(log.lines(), 0);
        assert_eq!(log.stats(&utc(), T0).playbacks, 0);
        assert_eq!(log.stats(&utc(), T0).bytes, 0);
    }

    #[test]
    fn the_oldest_playback_goes_once_three_are_kept() {
        let log = PlayerLog::new();
        for n in 1..=4 {
            log.begin(&n.to_string(), &["Moonlit Harbor".into()], T0 + n * 1000);
            line(&log, T0 + n * 1000 + 1, &format!("line of {n}"));
        }
        let kept = texts(&log);
        assert_eq!(kept.len(), PLAYBACKS);
        assert_eq!(
            kept.iter().map(|p| p.episode.as_str()).collect::<Vec<_>>(),
            ["2", "3", "4"],
            "oldest first, the first playback gone"
        );
        assert_eq!(kept[2].lines[0].message, "line of 4");
    }

    #[test]
    fn over_the_cap_the_oldest_playback_goes_before_any_line_of_the_newest() {
        let log = PlayerLog::new();
        let big = "x".repeat(500);
        log.begin("1", &[], T0);
        for _ in 0..2000 {
            line(&log, T0, &big);
        }
        log.begin("2", &[], T0 + 1);
        for _ in 0..2000 {
            line(&log, T0 + 1, &big);
        }
        let kept = texts(&log);
        assert_eq!(kept.len(), 1, "the first playback went as the second filled the cap");
        assert_eq!(kept[0].episode, "2");
        assert!(
            kept[0].dropped > 0,
            "the second, alone over the cap, dropped its oldest lines"
        );
        assert_eq!(kept[0].dropped as usize + kept[0].lines.len(), 2000);
        let bytes: usize = log.state().ring.bytes;
        assert!(bytes <= CAP_BYTES, "{bytes}");
    }

    #[test]
    fn a_single_playback_over_the_cap_says_how_many_lines_it_dropped() {
        let log = PlayerLog::new();
        log.begin("8", &["Moonlit Harbor".into()], T0);
        let big = "y".repeat(LINE_CHARS);
        for n in 0..3000 {
            line(&log, T0 + n, &format!("{n} {big}"));
        }
        let kept = &texts(&log)[0];
        assert!(kept.dropped > 0);
        assert!(
            kept.lines[0].message.starts_with(&format!("{} ", kept.dropped)),
            "oldest went"
        );
        let text = log.text(&BugReport::default(), &utc(), T0);
        assert!(
            text.contains(&format!("({} earlier lines were dropped", kept.dropped)),
            "{}",
            text.lines().take(8).collect::<Vec<_>>().join("\n")
        );
    }

    #[test]
    fn addresses_are_cut_as_lines_come_in() {
        let log = PlayerLog::new();
        log.begin("8", &[], T0);
        line(&log, T0, "open https://cdn.example/v/08.m3u8?token=abc123 now");
        assert_eq!(texts(&log)[0].lines[0].message, "open https://cdn.example now");
    }

    #[test]
    fn times_and_the_file_name_are_in_the_zone_given() {
        let zone = TimeZone::get("Europe/Copenhagen")
            .unwrap_or_else(|_| TimeZone::fixed(jiff::tz::offset(1)));
        assert_eq!(file_name(T0, &zone), "kaeri-player-log-2026-11-15-1522.txt");
        assert_eq!(file_name(T0, &utc()), "kaeri-player-log-2026-11-15-1422.txt");
        assert_eq!(clock_time(T0 + 7_412, &utc(), true), "14:22:07.412");
        assert_eq!(clock_time(T0, &utc(), false), "14:22");
    }

    #[test]
    fn the_mirror_is_read_back_after_a_crash() {
        let dir = tempfile::tempdir().unwrap();
        let first = PlayerLog::new();
        first.mirror_to(dir.path(), T0);
        first.begin("7", &["Moonlit Harbor".into()], T0);
        line(&first, T0 + 1, "streams found at https://cdn.example/a?sig=zzz");
        first.begin("8", &["Moonlit Harbor".into()], T0 + 2);
        line(&first, T0 + 3, "the player stopped");
        // The process ends without a word: only the file is left.
        drop(first);
        let file = std::fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert!(!file.contains("sig=zzz"), "addresses are cut in the file: {file}");
        let second = PlayerLog::new();
        second.mirror_to(dir.path(), T0 + 60_000);
        assert_eq!(second.lines(), 0, "nothing is read at the start");
        let stats = second.stats(&utc(), T0 + 60_000);
        assert_eq!(stats.playbacks, 2);
        let kept = texts(&second);
        assert_eq!(kept[0].lines[0].message, "streams found at https://cdn.example");
        assert_eq!(kept[1].lines[0].message, "the player stopped");
        // A playback after the start goes after what was read back.
        second.begin("9", &[], T0 + 70_000);
        assert_eq!(
            texts(&second).iter().map(|p| p.episode.as_str()).collect::<Vec<_>>(),
            ["7", "8", "9"]
        );
    }

    #[test]
    fn the_mirror_stays_under_its_cap() {
        let dir = tempfile::tempdir().unwrap();
        let log = PlayerLog::new();
        log.mirror_to(dir.path(), T0);
        log.begin("1", &[], T0);
        let big = "z".repeat(LINE_CHARS);
        for n in 0..6000 {
            line(&log, T0 + n, &big);
        }
        let size = std::fs::metadata(dir.path().join(FILE)).unwrap().len();
        assert!(size <= FILE_CAP, "{size}");
        // What it holds is what is kept.
        let again = PlayerLog::new();
        again.mirror_to(dir.path(), T0);
        assert_eq!(again.stats(&utc(), T0).playbacks, 1);
        assert_eq!(again.lines(), log.lines());
        assert_eq!(texts(&again)[0].dropped, texts(&log)[0].dropped);
    }

    #[test]
    fn a_mirror_thirty_days_old_is_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let log = PlayerLog::new();
        log.mirror_to(dir.path(), T0);
        log.begin("1", &[], T0);
        line(&log, T0, "old");
        drop(log);
        // Read back on day 29, it is kept.
        let later = PlayerLog::new();
        later.mirror_to(dir.path(), T0);
        assert_eq!(later.stats(&utc(), T0 + 29 * DAY_MS).playbacks, 1);
        drop(later);
        // Its newest line is past thirty days: read back as nothing, and written over empty.
        let past = PlayerLog::new();
        past.mirror_to(dir.path(), T0);
        assert_eq!(past.stats(&utc(), T0 + 31 * DAY_MS).playbacks, 0);
        assert!(!std::fs::read_to_string(dir.path().join(FILE)).unwrap().contains("old"));
        // By the file's own age, it is deleted as the mirror is set up.
        let now = crate::crash::now_ms();
        let aged = PlayerLog::new();
        aged.mirror_to(dir.path(), now + 31 * DAY_MS);
        assert!(!dir.path().join(FILE).exists());
    }

    #[test]
    fn a_foreign_file_is_left_out_and_written_over() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE), "not ours\nL\t1\tI\tplayer\tsecret\n").unwrap();
        let log = PlayerLog::new();
        log.mirror_to(dir.path(), T0);
        assert_eq!(log.stats(&utc(), T0).playbacks, 0);
        let file = std::fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert_eq!(file, format!("{HEADER}\n"));
    }

    #[test]
    fn a_private_playback_keeps_neither_its_title_nor_its_lines() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let log = PlayerLog::new();
        log.mirror_to(dir.path(), T0);
        log.begin("3", &["Moonlit Harbor".into()], T0);
        line(&log, T0 + 1, "opened in 40 ms");
        // The private playback: held, as the free `begin` holds it.
        log.hold();
        line(&log, T0 + 2_000, "Playing: /media/Hidden Lantern Garden/02.mkv");
        line(&log, T0 + 2_001, "title=Hidden Lantern Garden");
        let kept = texts(&log);
        assert_eq!(kept.len(), 1, "the private playback is not kept");
        assert_eq!(kept[0].lines.len(), 1);
        let report = BugReport::default();
        let text = log.text(&report, &utc(), T0 + 3_000);
        assert!(!text.contains("Hidden Lantern Garden"), "{text}");
        let file = std::fs::read_to_string(dir.path().join(FILE)).unwrap_or_default();
        assert!(!file.contains("Hidden Lantern Garden"), "{file}");
        // The next playback that is not private is kept again.
        log.begin("4", &["Moonlit Harbor".into()], T0 + 4_000);
        line(&log, T0 + 4_001, "opened in 35 ms");
        assert_eq!(texts(&log).len(), 2);
    }

    #[test]
    fn the_layer_takes_out_what_private_mode_has_learnt() {
        // A word only this test teaches the process's redactor.
        crate::private::redact::word("Quiet Bellflower Annex");
        let log = Arc::new(PlayerLog::new());
        let subscriber = tracing_subscriber::registry()
            .with(Keeper { log: Arc::clone(&log) }.with_filter(filter_fn(wanted)));
        tracing::subscriber::with_default(subscriber, || {
            log.begin("2", &[], T0);
            tracing::info!(target: "app", source = "mpv", "[cplayer] title: Quiet Bellflower Annex 02");
        });
        let kept = &texts(&log)[0];
        assert_eq!(kept.lines.len(), 1);
        assert!(!kept.lines[0].message.contains("Bellflower"), "{}", kept.lines[0].message);
        assert!(kept.lines[0].message.contains(crate::private::redact::HIDDEN));
    }

    #[test]
    fn the_layer_keeps_playback_lines_and_the_apps_mpv_lines_only() {
        let log = Arc::new(PlayerLog::new());
        let subscriber = tracing_subscriber::registry()
            .with(Keeper { log: Arc::clone(&log) }.with_filter(filter_fn(wanted)));
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "kaeri_engine::playback::resolve", "before any playback");
            log.begin("8", &["Moonlit Harbor".into()], T0);
            tracing::info!(target: "kaeri_engine::playback::resolve", streams = 3, "resolved");
            tracing::warn!(target: "kaeri_engine::playback::proxy::upstream", "segment 212 failed");
            tracing::info!(target: "kaeri_engine::playback::start", id = 21, episode = "8", "playing");
            tracing::info!(target: "app", source = "mpv", "[cplayer] Playing: https://cdn.example/v.m3u8?t=1");
            tracing::info!(target: "app", source = "player", "first_frame_ms=1290");
            tracing::info!(target: "app", source = "store", "a store line");
            tracing::debug!(target: "app", source = "mpv", "a debug line");
            tracing::info!(target: "kaeri_engine::sync", "a sync line");
            tracing::info!(target: "mpv", "[ffmpeg] hls: opening");
        });
        let kept = &texts(&log)[0];
        let said: Vec<String> = kept
            .lines
            .iter()
            .map(|line| format!("{} {} {}", line.level, line.source, line.message))
            .collect();
        assert_eq!(
            said,
            [
                "INFO resolve resolved streams=3",
                "WARN proxy segment 212 failed",
                "INFO player playing id=[removed] episode=8",
                "INFO mpv [cplayer] Playing: https://cdn.example",
                "INFO player first_frame_ms=1290",
                "INFO mpv [ffmpeg] hls: opening",
            ]
        );
    }

    #[test]
    fn the_engines_private_strings_are_removed_from_the_text() {
        use kaeri_model::{
            servers::{CheckedServer, ServerEvent},
            sync::SyncState,
        };
        let log = PlayerLog::new();
        log.remember(private_in(&Event::Sync(Box::new(SyncState {
            tracker: "anilist".into(),
            user: Some("viewer-handle".into()),
            ..SyncState::default()
        }))));
        log.remember(private_in(&Event::Server(ServerEvent::Checked(CheckedServer {
            id: "s1".into(),
            name: "Den Shelf".into(),
            version: "10.11".into(),
            address: "http://shelf.lan:8096".into(),
            quick_connect: false,
        }))));
        log.remember(private_in(&Event::MatchSearchFailed {
            id: kaeri_core::ids::AnilistId::new(21),
            query: "harbor lights".into(),
            reason: kaeri_model::Msg::NoticeCastNoDevice,
        }));
        log.remember(["LAMP-8812".to_owned()]);
        log.begin("8", &["Moonlit Harbor".into(), "The Lantern Festival".into()], T0);
        line(&log, T0, "viewer-handle joined LAMP-8812 to watch Moonlit Harbor");
        line(&log, T0, "[osd] The Lantern Festival");
        line(&log, T0, "fetching from http://shelf.lan:8096 (Den Shelf) for harbor lights");
        let text = log.text(&BugReport::default(), &utc(), T0);
        for leak in [
            "viewer-handle",
            "LAMP-8812",
            "shelf.lan",
            "Den Shelf",
            "harbor lights",
            "Moonlit",
            "Lantern",
        ] {
            assert!(!text.contains(leak), "{leak} in {text}");
        }
        assert!(text.contains("── Playback 1 of 1 · Episode 8 · 14:22 ──"), "{text}");
    }

    #[test]
    fn the_stats_count_the_redacted_file_and_follow_changes() {
        let log = PlayerLog::new();
        log.begin("8", &[], T0);
        line(&log, T0, "one");
        let first = log.stats(&utc(), T0);
        assert_eq!(first.playbacks, 1);
        assert_eq!(first.bytes, log.text(&BugReport::default(), &utc(), T0).len() as u64);
        line(&log, T0, "two");
        assert!(log.stats(&utc(), T0).bytes > first.bytes);
    }

    #[test]
    fn the_fixture_fills_the_row_and_says_the_title_nowhere() {
        let log = PlayerLog::new();
        fixture_into(&log, 3, T0);
        let stats = log.stats(&utc(), T0);
        assert_eq!(stats.playbacks, 3);
        assert!((200_000..600_000).contains(&stats.bytes), "{}", stats.bytes);
        let text = log.text(&BugReport::default(), &utc(), T0);
        assert!(!text.contains("Stardust"), "titles are removed");
        assert!(!text.contains("127.0.0.1"), "addresses are cut");
        // Numbered oldest first, listed newest first.
        let newest = text.find("── Playback 3 of 3 · Episode 3").expect("the newest playback");
        let oldest = text.find("── Playback 1 of 3 · Episode 1").expect("the oldest playback");
        assert!(newest < oldest, "newest first");
        assert!(text.contains("the stream stopped"));
        log.clear(T0);
        assert_eq!(log.stats(&utc(), T0).playbacks, 0, "zero is the empty state");
    }
}
