//! "Report a bug": the link to a pre-filled GitHub issue, the same text on every front end.
//!
//! There is no server. A front end gathers a [`BugReport`] (what it knows about itself, the
//! sources the engine lists, and the last error if the report starts from one), and [`issue_url`]
//! returns the address of a new issue on the project's tracker with the issue form's diagnostics
//! field filled in. The reporter signs in to GitHub there, writes what happened, and submits.
//!
//! The issue is public, so nothing private may reach it. Every free-form string goes through
//! [`redact`]: addresses, hostnames, IPs, file paths, tokens, emails, quoted text (a show's title
//! in an error) and anything the front end names as private are replaced by a generic word, and
//! sources are counted, never named.
//!
//! The text is bounded so the address stays under [`MAX_URL_BYTES`]: the error's message is cut
//! first.
//!
//! The player log is a file of its own, never in the address or in Copy report: the reporter
//! saves it and attaches it to the issue by hand. [`player_log_text`] is its text, redacted with
//! the same rules and headed with the report's summary.

use crate::crash::{CrashPlace, CrashRecord};

/// The project's tracker, where the link goes.
pub const REPO: &str = "bitknox/kaeri";
/// The issue form in `.github/ISSUE_TEMPLATE`, by file name.
pub const TEMPLATE: &str = "bug_report.yml";
/// The form field the summary fills. The form's other fields, "what_happened" and "steps", are
/// written by the reporter.
pub const DIAGNOSTICS_FIELD: &str = "diagnostics";
/// The most the address may be, in bytes. GitHub refuses a longer one with a 414.
pub const MAX_URL_BYTES: usize = 7_500;

/// How long the error's message may be before the address is measured, in characters.
const MESSAGE_CHARS: usize = 400;
/// How long any short field may be, in characters.
const FIELD_CHARS: usize = 60;

/// What kind of device the app runs on, as a bug report says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum DeviceClass {
    #[default]
    Computer,
    Phone,
    Tablet,
    Tv,
    Terminal,
}

impl DeviceClass {
    pub const fn word(self) -> &'static str {
        match self {
            Self::Computer => "computer",
            Self::Phone => "phone",
            Self::Tablet => "tablet",
            Self::Tv => "tv",
            Self::Terminal => "terminal",
        }
    }
}

/// The error a report starts from.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct BugError {
    /// What sort of failure: "network", "playback", "start". A short word the front end picks.
    pub kind: String,
    /// What it said. Redacted before it is used.
    pub message: String,
}

/// What a report is made from.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct BugReport {
    /// The app's version, such as "0.10.0".
    pub app_version: String,
    /// The system: "macOS", "iOS", "iPadOS", "Android", "Terminal".
    pub platform: String,
    /// The system's version, such as "26.1".
    pub os_version: String,
    pub device: DeviceClass,
    /// How the app was built: "release", "debug" or "fixtures".
    pub build: String,
    /// The sources the engine lists. They are counted, never named.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub providers: Vec<crate::sources::ProviderRow>,
    /// The error the report starts from, if any.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub error: Option<BugError>,
    /// Strings to remove wherever they appear: config values marked secret, and the title of the
    /// show the error was about.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub private: Vec<String>,
    /// The ids of the saved crashes to include. The engine's store holds them; the front end
    /// asks for the report's text or address through the calls that read them.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub crashes: Vec<String>,
    /// Whether Player log is ticked: the summary then says the log is attached by hand. The log
    /// itself never rides in the address or in Copy report.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub player_log: bool,
}

/// The [`BugError::kind`] of an error a playback ended in: a report opened from one starts with
/// Player log ticked (see [`player_log_ticked`]).
pub const PLAYBACK_ERROR: &str = "playback";

/// What the player log holds, for the Player log row: "The last 3 playbacks · 412 KB".
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct PlayerLogStats {
    /// How many playbacks it holds: 0 before anything has played, when the row is disabled.
    pub playbacks: u32,
    /// The size of the redacted file, in bytes.
    pub bytes: u64,
    /// What the saved file is called, in local time: `kaeri-player-log-2026-11-15-1432.txt`.
    pub file_name: String,
}

/// One line of the player log as the engine kept it, its addresses already cut: when, how much
/// it mattered, which part said it, and what it said.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LoggedLine {
    /// Local time with milliseconds: `14:21:07.412`.
    pub time: String,
    /// `ERROR`, `WARN` or `INFO`.
    pub level: String,
    /// `resolve`, `proxy`, `player` or `mpv`.
    pub source: String,
    pub message: String,
}

/// One playback in the player log, oldest line first.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LoggedPlayback {
    /// The episode's number as the playback named it: `8`.
    pub episode: String,
    /// When it started, in local time: `14:21`.
    pub started: String,
    /// The show's title and the episode's own, removed wherever they appear.
    pub titles: Vec<String>,
    pub lines: Vec<LoggedLine>,
    /// How many of its oldest lines were dropped to keep the log under its cap.
    pub dropped: u64,
}

/// The longest a line of the player log may be, in characters.
const LOG_LINE_CHARS: usize = 600;

/// The player log's file: the report's summary (versions, system, build and sources), then each
/// playback, newest first, every line redacted.
///
/// Removed wherever they appear: the report's private strings and sources, as in the summary, the
/// playbacks' titles, and `private`, what the engine knows to be private (tracker account names,
/// synced device names, party rooms, store links, server addresses, search words). Then
/// [`redact`]'s rules take the addresses, hosts, paths, tokens and quoted text that are left.
pub fn player_log_text(
    report: &BugReport,
    playbacks: &[LoggedPlayback],
    private: &[String],
) -> String {
    let mut remove = names(report);
    for playback in playbacks {
        remove.extend(playback.titles.iter().map(|title| (title.clone(), "[title]")));
    }
    remove.extend(private.iter().map(|text| (text.clone(), "[removed]")));
    let mut out = head_lines(report).join("\n");
    out.push('\n');
    let total = playbacks.len();
    for (n, playback) in playbacks.iter().enumerate().rev() {
        out.push('\n');
        let mut heading = format!("── Playback {} of {total}", n + 1);
        let episode = cut(&clean(&redact(&playback.episode, &remove)), 20);
        if !episode.is_empty() {
            heading.push_str(&format!(" · Episode {episode}"));
        }
        if !playback.started.is_empty() {
            heading.push_str(&format!(" · {}", playback.started));
        }
        heading.push_str(" ──\n");
        out.push_str(&heading);
        match playback.dropped {
            0 => {}
            1 => out.push_str("(1 earlier line was dropped to keep the log small)\n"),
            n => out
                .push_str(&format!("({n} earlier lines were dropped to keep the log small)\n")),
        }
        for line in &playback.lines {
            let message: String = redact(&line.message, &remove)
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .collect();
            let message = cut(message.trim_end(), LOG_LINE_CHARS);
            out.push_str(&format!(
                "{} {:<5} {:<8} {message}\n",
                line.time, line.level, line.source
            ));
        }
    }
    out
}

/// Whether Player log starts ticked as Report opens: from a playback error (an error of kind
/// [`PLAYBACK_ERROR`]) or with a crash in the player among `crashes`. From anywhere else, such as
/// Settings, it starts unticked.
pub fn player_log_ticked(report: &BugReport, crashes: &[CrashRecord]) -> bool {
    report.error.as_ref().is_some_and(|error| error.kind.trim() == PLAYBACK_ERROR)
        || crashes.iter().any(|crash| crash.place == CrashPlace::Player)
}

/// `text` with every address in it cut to its scheme and host, as the app log keeps mpv's lines:
/// a stream's address carries its signed token in its path or query. An address is a scheme and
/// `://`, and everything up to a space or a quote.
pub fn cut_addresses(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("://") {
        // The scheme runs back from `://` over letters, digits, `+`, `.` and `-`, and starts
        // with a letter.
        let before = &rest[..at];
        let start = before
            .char_indices()
            .rev()
            .take_while(|(_, c)| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
            .last()
            .map_or(at, |(i, _)| i);
        let start = before[start..]
            .char_indices()
            .find(|(_, c)| c.is_ascii_alphabetic())
            .map_or(at, |(i, _)| start + i);
        if start == at {
            out.push_str(&rest[..at + 3]);
            rest = &rest[at + 3..];
            continue;
        }
        let end = rest[at + 3..]
            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>'))
            .map_or(rest.len(), |i| at + 3 + i);
        out.push_str(&rest[..start]);
        out.push_str(&cut_address(&rest[start..end]));
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// `https://cdn.example`, from `https://cdn.example/a/b.m3u8?token=...`: the scheme, the host and
/// the port, never the credentials, the path or the query.
pub fn cut_address(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://").filter(|(s, _)| !s.is_empty()) else {
        return if url.starts_with('/') { "a local file".into() } else { "an address".into() };
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.rsplit('@').next().unwrap_or_default();
    format!("{scheme}://{host}")
}

/// The form field the crash summaries fill.
pub const CRASH_FIELD: &str = "crash";

/// The address of a new issue with the form's diagnostics filled in, at most [`MAX_URL_BYTES`].
pub fn issue_url(report: &BugReport) -> String {
    issue_url_capped(report, &[], MAX_URL_BYTES)
}

/// [`issue_url`] with the crash summaries of `crashes` in the form's crash field.
pub fn issue_url_with(report: &BugReport, crashes: &[CrashRecord]) -> String {
    issue_url_capped(report, crashes, MAX_URL_BYTES)
}

/// The address cut to fit `max_bytes`: the TV's QR code takes 900, so it stays readable from
/// the sofa. The error's message is cut first, then the crashes' frames, then the crashes to
/// their kind line, and last the text itself.
pub fn issue_url_capped(
    report: &BugReport,
    crashes: &[CrashRecord],
    max_bytes: usize,
) -> String {
    let mut chars = MESSAGE_CHARS;
    loop {
        let url = build(report, crashes, chars, true);
        if url.len() <= max_bytes {
            return url;
        }
        if chars == 0 {
            break;
        }
        chars /= 2;
    }
    // Without frames, then with only the newest crash, then cut to the byte.
    for (limit, frames) in [(crashes.len(), false), (1, false)] {
        let url = build(report, &crashes[..limit.min(crashes.len())], 0, frames);
        if url.len() <= max_bytes {
            return url;
        }
    }
    let mut url = build(report, &crashes[..crashes.len().min(1)], 0, false);
    // The address is ASCII, so it can be cut at the byte; a cut escape goes with it.
    url.truncate(max_bytes);
    if let Some(percent) = url.rfind('%')
        && url.len() - percent < 3
    {
        url.truncate(percent);
    }
    url
}

/// The exact text the report's view shows, and Copy report puts on the clipboard: the summary,
/// then each crash whole.
pub fn report_text(report: &BugReport, crashes: &[CrashRecord]) -> String {
    let mut text = summary_with(report, MESSAGE_CHARS);
    for crash in crashes {
        text.push_str("\n\n");
        text.push_str(&crate::crash::crash_text(crash));
    }
    text
}

/// A short redacted field: for the crash records, which share the report's rules.
pub(crate) fn short_text(text: &str, chars: usize) -> String {
    cut(&clean(&redact(text, &[])), chars)
}

/// The summary the issue carries, as the reporter reads it before submitting.
pub fn summary(report: &BugReport) -> String {
    summary_with(report, MESSAGE_CHARS)
}

fn build(
    report: &BugReport,
    crashes: &[CrashRecord],
    message_chars: usize,
    frames: bool,
) -> String {
    let mut url = format!("https://github.com/{REPO}/issues/new?template={TEMPLATE}");
    if let Some(error) = &report.error {
        let kind = short(&error.kind, report);
        if !kind.is_empty() {
            url.push_str("&title=");
            url.push_str(&encode(&format!("Bug: {kind}")));
        }
    }
    url.push('&');
    url.push_str(DIAGNOSTICS_FIELD);
    url.push('=');
    url.push_str(&encode(&summary_with(report, message_chars)));
    if !crashes.is_empty() {
        let text: Vec<String> = crashes
            .iter()
            .map(|c| {
                if frames {
                    crate::crash::crash_text(c)
                } else {
                    crate::crash::crash_summary_text(c)
                }
            })
            .collect();
        url.push('&');
        url.push_str(CRASH_FIELD);
        url.push('=');
        url.push_str(&encode(&text.join("\n\n")));
    }
    url
}

/// A short field of the report, or "unknown".
fn field(value: &str, report: &BugReport) -> String {
    let value = short(value, report);
    if value.is_empty() { "unknown".to_owned() } else { value }
}

/// The summary's first lines, which head the player log too: versions, system, build, sources.
fn head_lines(report: &BugReport) -> Vec<String> {
    let (built_in, plugins) = sources(&report.providers);
    vec![
        format!(
            "Kaeri {}, engine {}",
            field(&report.app_version, report),
            env!("CARGO_PKG_VERSION")
        ),
        format!(
            "{} {}, {}",
            field(&report.platform, report),
            field(&report.os_version, report),
            report.device.word()
        ),
        format!("Build: {}", field(&report.build, report)),
        format!("Sources: {}", sources_line(&built_in, plugins)),
    ]
}

fn summary_with(report: &BugReport, message_chars: usize) -> String {
    let field = |value: &str| field(value, report);
    let mut lines = head_lines(report);
    match &report.error {
        Some(error) => {
            lines.push(format!("Last error: {}", field(&error.kind)));
            let message = cut(&clean(&redact(&error.message, &names(report))), message_chars);
            if !message.is_empty() {
                lines.push(format!("Message: {message}"));
            }
        }
        None => lines.push("Last error: none".to_owned()),
    }
    if report.player_log {
        lines.push("Player log: attached by hand".to_owned());
    }
    lines.join("\n")
}

/// The built-in kinds that are on, and how many plugins are.
fn sources(rows: &[crate::sources::ProviderRow]) -> (Vec<String>, usize) {
    use crate::sources::ProviderOrigin;
    let mut built_in: Vec<String> = Vec::new();
    let mut plugins = 0;
    for row in rows.iter().filter(|r| r.usable && !r.disabled && !r.is_demo) {
        if row.origin == ProviderOrigin::BuiltIn {
            // The kind is a fixed word; an id may be one the user chose.
            let kind: String =
                row.kind.chars().filter(char::is_ascii_alphanumeric).take(20).collect();
            if !kind.is_empty() && !built_in.contains(&kind) {
                built_in.push(kind);
            }
        } else {
            plugins += 1;
        }
    }
    built_in.sort();
    (built_in, plugins)
}

fn sources_line(built_in: &[String], plugins: usize) -> String {
    let mut parts = built_in.to_vec();
    match plugins {
        0 => {}
        1 => parts.push("a plugin source".to_owned()),
        n => parts.push(format!("{n} plugin sources")),
    }
    if parts.is_empty() { "none".to_owned() } else { parts.join(", ") }
}

/// Strings that are removed wherever they appear: the front end's private ones, and every name,
/// id and host of a source that is not built in.
fn names(report: &BugReport) -> Vec<(String, &'static str)> {
    let mut out: Vec<(String, &'static str)> =
        report.private.iter().map(|p| (p.clone(), "[removed]")).collect();
    for row in &report.providers {
        for name in [&row.id, &row.name].into_iter().chain(row.hosts.iter()) {
            out.push((name.clone(), "a plugin source"));
        }
    }
    out
}

/// A short field: redacted, one line, bounded.
fn short(text: &str, report: &BugReport) -> String {
    cut(&clean(&redact(text, &names(report))), FIELD_CHARS)
}

/// Control characters and runs of space as one space.
fn clean(text: &str) -> String {
    text.split_whitespace()
        .map(|w| w.chars().filter(|c| !c.is_control()).collect::<String>())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn cut(text: &str, chars: usize) -> String {
    if text.chars().count() <= chars {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(chars).collect();
    out.truncate(out.trim_end().len());
    if chars > 0 {
        out.push_str("...");
    }
    out
}

/// Percent-encoding for a query value: every byte but an unreserved one.
fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

const EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "avi", "webm", "mov", "m3u8", "ts", "srt", "ass", "vtt", "torrent", "json",
    "toml", "db", "log", "txt", "wasm", "jpg", "png", "zip", "sqlite", "plist", "xml", "yml",
];
const SECRET_KEYS: &[&str] = &[
    "token",
    "key",
    "secret",
    "password",
    "passwd",
    "pass",
    "auth",
    "authorization",
    "session",
    "cookie",
    "signature",
    "sig",
    "code",
    "invite",
];
const SECRET_PREFIXES: &[&str] =
    &["ghp_", "gho_", "ghs_", "github_pat_", "sk-", "xox", "akia", "eyj"];

/// `text` with everything private in it replaced by a generic word.
///
/// First the given strings (case-insensitively, whatever they sit inside), then quoted text, then
/// each word: links, emails, paths, files, addresses, hostnames and anything that looks like a
/// token or key.
pub fn redact(text: &str, remove: &[(String, &str)]) -> String {
    let mut text = text.to_owned();
    let mut remove: Vec<&(String, &str)> =
        remove.iter().filter(|(s, _)| s.trim().chars().count() >= 2).collect();
    remove.sort_by_key(|(s, _)| std::cmp::Reverse(s.len()));
    for (needle, with) in remove {
        text = replace_ci(&text, needle.trim(), with);
    }
    let text = redact_quoted(&text);
    redact_words(&text)
}

fn replace_ci(text: &str, needle: &str, with: &str) -> String {
    let (t, n) = (text.as_bytes(), needle.as_bytes());
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut last = 0;
    while i + n.len() <= t.len() {
        if t[i..i + n.len()].eq_ignore_ascii_case(n) {
            out.push_str(&text[last..i]);
            out.push_str(with);
            i += n.len();
            last = i;
        } else {
            i += 1;
        }
    }
    out.push_str(&text[last..]);
    out
}

fn redact_quoted(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let close = match c {
            '"' => Some('"'),
            '\u{201C}' => Some('\u{201D}'),
            '\u{2018}' => Some('\u{2019}'),
            '`' => Some('`'),
            '\'' if i == 0 || !chars[i - 1].is_alphanumeric() => Some('\''),
            _ => None,
        };
        if let Some(close) = close {
            let end = (i + 1..chars.len()).find(|&j| {
                chars[j] == close
                    && (!matches!(close, '\'' | '\u{2019}')
                        || chars.get(j + 1).is_none_or(|n| !n.is_alphanumeric()))
            });
            if let Some(end) = end {
                out.push_str("[quoted]");
                i = end + 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

fn redact_words(text: &str) -> String {
    let mut out = String::new();
    let mut prev = String::new();
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String, prev: &mut String| {
        if !word.is_empty() {
            out.push_str(&redact_word(word, prev));
            *prev = word.to_lowercase();
            word.clear();
        }
    };
    for c in text.chars() {
        if c.is_whitespace() {
            flush(&mut word, &mut out, &mut prev);
            out.push(c);
        } else {
            word.push(c);
        }
    }
    flush(&mut word, &mut out, &mut prev);
    out
}

fn redact_word(word: &str, prev: &str) -> String {
    let is_lead = |c: char| matches!(c, '(' | '[' | '{' | '<' | '"' | '\'' | '`');
    let is_trail = |c: char| {
        matches!(
            c,
            ')' | ']' | '}' | '>' | '"' | '\'' | '`' | ',' | ';' | ':' | '!' | '?' | '.'
        )
    };
    let start = word.len() - word.trim_start_matches(is_lead).len();
    let core_end = word.trim_end_matches(is_trail).len().max(start);
    let (lead, core, trail) = (&word[..start], &word[start..core_end], &word[core_end..]);
    if core.is_empty() {
        return word.to_owned();
    }
    let core = if matches!(prev, "bearer" | "basic") && core.len() >= 8 {
        "[secret]".to_owned()
    } else {
        redact_core(core)
    };
    format!("{lead}{core}{trail}")
}

fn redact_core(core: &str) -> String {
    // key=value and a=b&c=d: each part on its own, a sensitive key takes its whole value.
    if core.contains('=') && !is_link(core) {
        return core
            .split('&')
            .map(|pair| match pair.split_once('=') {
                Some((k, v)) => {
                    let key = k.to_lowercase();
                    // The key is redacted too: a path or an address can sit before the `=`.
                    let k = redact_core(k);
                    if SECRET_KEYS.iter().any(|s| key.contains(s)) && !v.is_empty() {
                        format!("{k}=[secret]")
                    } else {
                        format!("{k}={}", redact_core(v))
                    }
                }
                None => redact_core(pair),
            })
            .collect::<Vec<_>>()
            .join("&");
    }
    if is_link(core) {
        return "[link]".into();
    }
    if core.split_once('@').is_some_and(|(_, domain)| domain.contains('.')) {
        return "[email]".into();
    }
    if core.contains('/') || core.contains('\\') || core.starts_with('~') || is_drive(core) {
        return "[path]".into();
    }
    let host_part = strip_port(core);
    if is_ip(host_part) || is_ip(core) {
        return "[address]".into();
    }
    if let Some((_, ext)) = core.rsplit_once('.') {
        let ext = ext.to_lowercase();
        if EXTENSIONS.contains(&ext.as_str()) {
            return "[file]".into();
        }
    }
    if is_host(host_part) {
        return "[host]".into();
    }
    if looks_secret(core) {
        return "[secret]".into();
    }
    core.to_owned()
}

fn is_link(s: &str) -> bool {
    let lower = s.to_lowercase();
    s.contains("://") || ["www.", "mailto:", "magnet:"].iter().any(|p| lower.starts_with(p))
}

fn is_drive(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 2
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && b.get(2).is_some_and(|c| matches!(c, b'\\' | b'/'))
}

fn strip_port(s: &str) -> &str {
    // `[::1]:7000`, an IPv6 address with a port, its opening bracket already taken as a lead.
    if let Some((host, port)) = s.rsplit_once("]:")
        && !port.is_empty()
        && port.bytes().all(|b| b.is_ascii_digit())
    {
        return host.trim_start_matches('[');
    }
    match s.rsplit_once(':') {
        Some((host, port))
            if !host.contains(':')
                && !port.is_empty()
                && port.bytes().all(|b| b.is_ascii_digit()) =>
        {
            host
        }
        _ => s,
    }
}

fn is_ip(s: &str) -> bool {
    let s = s.trim_matches(|c| c == '[' || c == ']');
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() == 4
        && parts
            .iter()
            .all(|p| (1..=3).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()))
    {
        return true;
    }
    let colons = s.matches(':').count();
    colons >= 2
        && (s.contains("::") || colons == 7)
        && s.chars().all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.')
}

fn is_host(s: &str) -> bool {
    let labels: Vec<&str> = s.split('.').collect();
    labels.len() >= 2
        && labels
            .iter()
            .all(|l| !l.is_empty() && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
        && labels
            .last()
            .is_some_and(|l| l.len() >= 2 && l.bytes().all(|b| b.is_ascii_alphabetic()))
}

fn looks_secret(s: &str) -> bool {
    let lower = s.to_lowercase();
    let body = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '+' | '=');
    if !s.chars().all(body) && !s.contains('.') {
        return false;
    }
    if SECRET_PREFIXES.iter().any(|p| lower.starts_with(p)) && s.len() >= 10 {
        return true;
    }
    // A JWT: three dot-separated runs.
    if s.split('.').count() == 3
        && s.len() >= 20
        && s.split('.').all(|p| !p.is_empty() && p.chars().all(body))
    {
        return true;
    }
    if !s.chars().all(body) {
        return false;
    }
    let digits = s.bytes().any(|b| b.is_ascii_digit());
    let hex = s.len() >= 16 && s.bytes().all(|b| b.is_ascii_hexdigit());
    hex || (s.len() >= 20 && digits) || s.len() >= 32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::{ProviderOrigin, ProviderRow};
    use std::{collections::BTreeSet, fs, path::Path};

    fn red(s: &str) -> String {
        redact(s, &[])
    }

    fn row(id: &str, kind: &str, origin: ProviderOrigin) -> ProviderRow {
        ProviderRow {
            id: id.into(),
            name: format!("{id} name"),
            kind: kind.into(),
            usable: true,
            hosts: vec![format!("{id}.example")],
            origin,
            ..ProviderRow::default()
        }
    }

    fn report() -> BugReport {
        BugReport {
            app_version: "0.10.0".into(),
            platform: "macOS".into(),
            os_version: "26.1".into(),
            device: DeviceClass::Computer,
            build: "release".into(),
            providers: vec![
                row("torrents", "torrent", ProviderOrigin::BuiltIn),
                row("moonlit", "plugin", ProviderOrigin::ByHand),
                row("lantern", "plugin", ProviderOrigin::Store { id: "s".into(), name: None }),
            ],
            error: Some(BugError { kind: "playback".into(), message: "no stream".into() }),
            private: vec![],
            crashes: vec![],
            player_log: false,
        }
    }

    #[test]
    fn links_are_removed() {
        assert_eq!(
            red("GET https://cdn.moon.example/v/a.m3u8?t=abc failed"),
            "GET [link] failed"
        );
        assert_eq!(red("see (http://x.example/p)."), "see ([link]).");
        assert_eq!(red("open www.moon.example now"), "open [link] now");
        assert_eq!(red("magnet:?xt=urn:btih:abc"), "[link]");
    }

    #[test]
    fn hosts_and_addresses_are_removed() {
        assert_eq!(red("cannot reach api.moon.example:8443."), "cannot reach [host].");
        assert_eq!(red("dns for moon.example failed"), "dns for [host] failed");
        assert_eq!(red("connect 192.168.1.20:8080 refused"), "connect [address] refused");
        assert_eq!(red("peer 2001:db8::1 gone"), "peer [address] gone");
        assert_eq!(red("::1 refused"), "[address] refused");
    }

    #[test]
    fn paths_and_files_are_removed() {
        assert_eq!(red("cannot open /Users/someone/Library/db.sqlite"), "cannot open [path]");
        assert_eq!(red("read ~/notes failed"), "read [path] failed");
        assert_eq!(red(r"missing C:\Users\x\a.txt"), "missing [path]");
        assert_eq!(red("wrote Episode.Name.S01E02.mkv"), "wrote [file]");
        assert_eq!(red("bad config.toml"), "bad [file]");
    }

    #[test]
    fn emails_are_removed() {
        assert_eq!(red("mail me@mail.example please"), "mail [email] please");
    }

    #[test]
    fn tokens_and_keys_are_removed() {
        assert_eq!(red("Authorization: Bearer abcdefghijkl"), "Authorization: Bearer [secret]");
        assert_eq!(red("token=abc123 ok"), "token=[secret] ok");
        assert_eq!(red("api_key=zz&mode=fast"), "api_key=[secret]&mode=fast");
        assert_eq!(red("ghp_abcdefghijklmnop"), "[secret]");
        assert_eq!(red("sk-abcdefghij"), "[secret]");
        assert_eq!(red("hash 0123456789abcdef0123456789abcdef01234567"), "hash [secret]");
        assert_eq!(red("id a1b2c3d4e5f6g7h8i9j0k1"), "id [secret]");
        assert_eq!(red("eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ4In0.c2lnbmF0dXJl"), "[secret]");
    }

    #[test]
    fn quoted_text_is_removed_so_a_title_cannot_leak() {
        assert_eq!(
            red(r#"no streams for "Lantern Harbor" episode 3"#),
            "no streams for [quoted] episode 3"
        );
        assert_eq!(red("no match for 'Lantern Harbor' today"), "no match for [quoted] today");
        assert_eq!(red("no match for \u{201C}Lantern Harbor\u{201D}"), "no match for [quoted]");
        // An apostrophe inside a word opens nothing.
        assert_eq!(red("it didn't work, can't retry"), "it didn't work, can't retry");
    }

    #[test]
    fn given_strings_go_wherever_they_sit_in_any_case() {
        let out = redact(
            "Hunting LANTERN harbor, lantern-harborx",
            &[("lantern harbor".into(), "[removed]")],
        );
        assert_eq!(out, "Hunting [removed], lantern-harborx");
        let out = redact(
            "pw=hunter2hunter2 hunter2hunter2",
            &[("hunter2hunter2".into(), "[removed]")],
        );
        assert_eq!(out, "pw=[removed] [removed]");
        // One character is no secret; it would eat every letter.
        assert_eq!(redact("a b", &[("a".into(), "[removed]")]), "a b");
    }

    #[test]
    fn plain_words_and_versions_stay() {
        let plain = "stream stalled after 3 retries on v0.10.3 at 12:30 (code 5)";
        assert_eq!(red(plain), plain);
        assert_eq!(red("e.g. later, i.e. now"), "e.g. later, i.e. now");
    }

    #[test]
    fn sources_are_counted_and_never_named() {
        let text = summary(&report());
        assert!(text.contains("Sources: torrent, 2 plugin sources"), "{text}");
        for name in ["moonlit", "lantern", "moon"] {
            assert!(!text.to_lowercase().contains(name), "{name} in {text}");
        }
        let mut one = report();
        one.providers.truncate(2);
        assert!(summary(&one).contains("torrent, a plugin source"));
        let mut off = report();
        off.providers[1].disabled = true;
        assert!(summary(&off).contains("torrent, a plugin source"));
        off.providers.clear();
        assert!(summary(&off).contains("Sources: none"));
    }

    #[test]
    fn a_source_named_inside_an_error_becomes_a_plugin_source() {
        let mut r = report();
        r.error = Some(BugError {
            kind: "source".into(),
            message: "Moonlit name returned 502 for lantern".into(),
        });
        let text = summary(&r);
        assert!(
            text.contains("Message: a plugin source returned 502 for a plugin source"),
            "{text}"
        );
        assert!(
            !text.to_lowercase().contains("moonlit")
                && !text.to_lowercase().contains("lantern")
        );
    }

    #[test]
    fn the_summary_carries_the_versions_platform_device_build_and_error() {
        let text = summary(&report());
        assert!(
            text.starts_with(&format!("Kaeri 0.10.0, engine {}\n", env!("CARGO_PKG_VERSION")))
        );
        assert!(text.contains("macOS 26.1, computer"));
        assert!(text.contains("Build: release"));
        assert!(text.contains("Last error: playback"));
        assert!(text.contains("Message: no stream"));
        let mut none = report();
        none.error = None;
        let text = summary(&none);
        assert!(text.contains("Last error: none") && !text.contains("Message"));
    }

    #[test]
    fn a_hostile_error_leaves_nothing_behind() {
        let mut r = report();
        r.private = vec!["s3cr3t-value".into(), "Lantern Harbor".into()];
        r.error = Some(BugError {
            kind: "net https://leak.example/x".into(),
            message: "GET https://moon.example/a?b=1 from 10.0.0.2 for Lantern Harbor \
                      using /home/me/x.json as me@mail.example with s3cr3t-value\nand\tstuff"
                .into(),
        });
        let url = issue_url(&r);
        let text = decode(&url);
        for leak in [
            "moon.example",
            "10.0.0.2",
            "Lantern",
            "/home",
            "mail.example",
            "s3cr3t",
            "leak.example",
        ] {
            assert!(!text.contains(leak), "{leak} in {text}");
        }
        assert!(!url.contains('\n') && !url.contains(' '));
    }

    #[test]
    fn the_url_stays_under_the_limit_by_trimming_the_message() {
        let mut r = report();
        r.error = Some(BugError { kind: "x".into(), message: "word ".repeat(5000) });
        let url = issue_url(&r);
        assert!(url.len() <= MAX_URL_BYTES, "{}", url.len());
        assert!(decode(&url).contains("Message: word"));
        // Multi-byte text encodes to more bytes; the limit still holds.
        r.error = Some(BugError { kind: "x".into(), message: "\u{3042}".repeat(5000) });
        assert!(issue_url(&r).len() <= MAX_URL_BYTES);
        // Short messages are kept whole.
        r.error = Some(BugError { kind: "x".into(), message: "short".into() });
        assert!(decode(&issue_url(&r)).contains("Message: short"));
    }

    #[test]
    fn a_report_without_an_error_has_no_title() {
        let mut r = report();
        r.error = None;
        let url = issue_url(&r);
        assert!(!url.contains("title="));
        assert!(url.starts_with(
            "https://github.com/bitknox/kaeri/issues/new?template=bug_report.yml&diagnostics="
        ));
        assert!(issue_url(&report()).contains("&title=Bug%3A%20playback&"));
    }

    fn decode(url: &str) -> String {
        query(url).into_iter().map(|(_, v)| v).collect::<Vec<_>>().join("\n")
    }

    fn query(url: &str) -> Vec<(String, String)> {
        let (_, q) = url.split_once('?').expect("a query");
        q.split('&')
            .map(|p| {
                let (k, v) = p.split_once('=').expect("a pair");
                (unescape(k), unescape(v))
            })
            .collect()
    }

    fn unescape(s: &str) -> String {
        let b = s.as_bytes();
        let mut out = Vec::new();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'%' {
                out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).expect("hex"));
                i += 3;
            } else {
                out.push(b[i]);
                i += 1;
            }
        }
        String::from_utf8(out).expect("utf8")
    }

    /// The `id:` of each input in the issue form.
    fn form_ids() -> BTreeSet<String> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.github/ISSUE_TEMPLATE")
            .join(TEMPLATE);
        let form = fs::read_to_string(path).expect("the issue form");
        form.lines()
            .filter_map(|l| l.trim().strip_prefix("id: "))
            .map(|id| id.trim().trim_matches('"').to_owned())
            .collect()
    }

    fn crash(n: i64) -> CrashRecord {
        CrashRecord {
            id: format!("c{n}"),
            at_ms: 1_793_394_840_000 + n,
            uptime_secs: 60,
            platform: "Mac".into(),
            os_version: "macOS 26.1".into(),
            app_version: "0.11.4".into(),
            arch: "arm64".into(),
            place: crate::crash::CrashPlace::Player,
            detail: "playing".into(),
            kind: crate::crash::CrashKind::Signal,
            reason: "EXC_BAD_ACCESS (SIGSEGV)".into(),
            frames: (0..30)
                .map(|offset| crate::crash::CrashFrame {
                    module: "libmpv.2.dylib".into(),
                    symbol: None,
                    offset: 0x1000 + offset,
                })
                .collect(),
            sources: 2,
            demo_on: true,
            origin: crate::crash::CrashOrigin::Saved,
        }
    }

    #[test]
    fn a_crash_report_stays_inside_the_address_limit_and_the_tvs_nine_hundred_bytes() {
        let crashes = [crash(1), crash(2), crash(3)];
        let url = issue_url_with(&report(), &crashes);
        assert!(url.len() <= MAX_URL_BYTES, "{}", url.len());
        assert!(url.contains("&crash="), "the crash goes in its own field");
        for cap in [900, 500, 200] {
            let url = issue_url_capped(&report(), &crashes, cap);
            assert!(url.len() <= cap, "{} over {cap}", url.len());
            assert!(url.starts_with("https://github.com/"), "{url}");
        }
        let tv = issue_url_capped(&report(), &crashes[..1], 900);
        assert!(decode(&tv).contains("Kind: crash"), "the kind line survives the cap");
    }

    #[test]
    fn the_report_text_is_the_summary_and_each_crash_whole() {
        let text = report_text(&report(), &[crash(1)]);
        assert!(text.starts_with(&summary(&report())));
        assert!(text.contains("\n\nKaeri crash report\n"));
        assert!(text.contains("libmpv.2.dylib"));
    }

    #[test]
    fn every_parameter_of_every_url_is_a_field_of_the_issue_form() {
        let ids = form_ids();
        assert!(
            ids.contains(DIAGNOSTICS_FIELD)
                && ids.contains("what_happened")
                && ids.contains("steps"),
            "{ids:?}"
        );
        let mut with_error = report();
        with_error.error = Some(BugError { kind: "start".into(), message: "x".into() });
        let mut without = report();
        without.error = None;
        for r in [with_error, without, BugReport::default()] {
            for (key, value) in query(&issue_url_with(&r, &[crash(1)])) {
                match key.as_str() {
                    "template" => assert_eq!(value, TEMPLATE),
                    "title" => assert!(value.starts_with("Bug: ")),
                    other => {
                        assert!(ids.contains(other), "{other} is not a form field: {ids:?}")
                    }
                }
            }
        }
    }

    #[test]
    fn a_ticked_player_log_says_so_in_the_summary_and_nowhere_else() {
        let mut r = report();
        assert!(!summary(&r).contains("Player log"));
        r.player_log = true;
        assert!(summary(&r).ends_with("\nPlayer log: attached by hand"), "{}", summary(&r));
        let text = report_text(&r, &[]);
        assert!(text.contains("Player log: attached by hand"));
        assert!(!text.contains("── Playback"), "Copy report never carries the log: {text}");
    }

    #[test]
    fn the_log_starts_ticked_after_a_playback_error_or_a_crash_in_the_player() {
        let mut r = report();
        r.error = Some(BugError { kind: PLAYBACK_ERROR.into(), message: "x".into() });
        assert!(player_log_ticked(&r, &[]));
        r.error = Some(BugError { kind: "message".into(), message: "x".into() });
        assert!(!player_log_ticked(&r, &[]));
        let mut player = crash(1);
        player.place = CrashPlace::Player;
        assert!(player_log_ticked(&r, &[player]));
        let mut home = crash(2);
        home.place = CrashPlace::Home;
        assert!(!player_log_ticked(&r, &[home]));
        r.error = None;
        assert!(!player_log_ticked(&r, &[]), "from Settings it starts unticked");
    }

    #[test]
    fn every_address_in_a_line_is_cut_to_its_host() {
        assert_eq!(
            cut_address("https://cdn.example/a/b.m3u8?token=secret"),
            "https://cdn.example"
        );
        assert_eq!(cut_address("http://user:pw@127.0.0.1:8123/m/x"), "http://127.0.0.1:8123");
        assert_eq!(cut_address("/home/viewer/episode.mkv"), "a local file");
        assert_eq!(cut_address("episode.mkv"), "an address");
        let line = "Opening https://cdn.example/seg/1.ts?sig=abc failed, retrying \"http://b.example:81/x\"";
        assert_eq!(
            cut_addresses(line),
            "Opening https://cdn.example failed, retrying \"http://b.example:81\""
        );
        assert_eq!(cut_addresses("no address here"), "no address here");
        assert_eq!(cut_addresses("odd ://thing"), "odd ://thing");
    }

    fn logged(time: &str, level: &str, source: &str, message: &str) -> LoggedLine {
        LoggedLine {
            time: time.into(),
            level: level.into(),
            source: source.into(),
            message: message.into(),
        }
    }

    /// A playback whose lines carry what the hostile plugin fixture
    /// (`plugins/test-hostile`) tries: a host it never declared, a credential it was never
    /// granted, its own name and id, and the title, signed links and paths around it.
    fn hostile_playback() -> LoggedPlayback {
        LoggedPlayback {
            episode: "8".into(),
            started: "14:21".into(),
            titles: vec!["Moonlit Harbor".into(), "The Lantern Festival".into()],
            lines: vec![
                logged(
                    "14:21:07.412",
                    "INFO",
                    "resolve",
                    "resolving a play id=[removed] episode=8 sources=[\"test-hostile\", \"torrents\"]",
                ),
                logged(
                    "14:21:07.500",
                    "WARN",
                    "resolve",
                    "denied: https://exfiltrate.example/steal is not an allowed host",
                ),
                logged(
                    "14:21:07.501",
                    "WARN",
                    "resolve",
                    "ESCAPED: config-get leaked \"credential-that-does-not-exist\"",
                ),
                logged(
                    "14:21:07.502",
                    "INFO",
                    "resolve",
                    "Hostile (test only) answered from allowed.example with 3 streams",
                ),
                logged("14:21:07.630", "INFO", "proxy", "listening on 127.0.0.1:52811"),
                logged(
                    "14:21:07.702",
                    "INFO",
                    "player",
                    "open https://cdn.allowed.example/v/Moonlit%20Harbor/08.m3u8?token=eyJhbGciOiJIUzI1NiJ9.abc.def",
                ),
                logged(
                    "14:21:07.940",
                    "INFO",
                    "mpv",
                    "[cplayer] Playing: Moonlit Harbor - 08 [1080p].mkv",
                ),
                logged(
                    "14:21:08.613",
                    "INFO",
                    "mpv",
                    "[cplayer]  (+) Subs  --sid=1 --slang=eng /Users/viewer/Movies/Moonlit Harbor/08.ass",
                ),
                logged("14:21:08.700", "INFO", "mpv", "[osd] The Lantern Festival"),
                logged(
                    "14:21:09.000",
                    "INFO",
                    "player",
                    "signed in as viewer-handle on [::1]:7000 room LAMP-8812 for viewer@mail.example",
                ),
                logged(
                    "14:33:58.004",
                    "ERROR",
                    "proxy",
                    "segment 212 failed after 3 tries (cdn.allowed.example)",
                ),
            ],
            dropped: 0,
        }
    }

    #[test]
    fn a_hostile_player_log_leaves_nothing_private_behind() {
        let mut r = report();
        r.providers.push(ProviderRow {
            id: "test-hostile".into(),
            name: "Hostile (test only)".into(),
            kind: "plugin".into(),
            usable: true,
            hosts: vec!["allowed.example".into()],
            origin: ProviderOrigin::ByHand,
            ..ProviderRow::default()
        });
        let private = vec!["viewer-handle".into(), "LAMP-8812".into()];
        let text = player_log_text(&r, &[hostile_playback()], &private);
        for leak in [
            "Moonlit",
            "Lantern",
            "exfiltrate.example",
            "allowed.example",
            "credential-that-does-not-exist",
            "test-hostile",
            "Hostile",
            "127.0.0.1",
            "52811",
            "eyJ",
            "token",
            "/Users",
            "08.ass",
            "viewer-handle",
            "LAMP-8812",
            "::1",
            "mail.example",
        ] {
            assert!(!text.contains(leak), "{leak} in\n{text}");
        }
        // What helps a report stays: the times, levels, sources and plain words.
        assert!(
            text.contains("14:33:58.004 ERROR proxy    segment 212 failed after 3 tries"),
            "{text}"
        );
        assert!(
            text.contains(
                "14:21:08.613 INFO  mpv      [cplayer]  (+) Subs  --sid=1 --slang=eng"
            ),
            "{text}"
        );
    }

    #[test]
    fn the_log_is_headed_by_the_summary_and_runs_newest_playback_first() {
        let mut r = report();
        r.player_log = true;
        let older = LoggedPlayback {
            episode: "7".into(),
            started: "13:48".into(),
            titles: vec!["Moonlit Harbor".into()],
            lines: vec![logged("13:48:02.118", "INFO", "resolve", "2 streams")],
            dropped: 0,
        };
        let newer = LoggedPlayback {
            episode: "8".into(),
            started: "14:21".into(),
            titles: vec!["Moonlit Harbor".into()],
            lines: vec![
                logged("14:21:07.412", "INFO", "resolve", "3 streams"),
                logged("14:21:08.902", "INFO", "player", "first_frame_ms=1290"),
            ],
            dropped: 1204,
        };
        let text = player_log_text(&r, &[older, newer], &[]);
        let engine = env!("CARGO_PKG_VERSION");
        assert_eq!(
            text,
            format!(
                "Kaeri 0.10.0, engine {engine}\n\
                 macOS 26.1, computer\n\
                 Build: release\n\
                 Sources: torrent, 2 plugin sources\n\
                 \n\
                 ── Playback 2 of 2 · Episode 8 · 14:21 ──\n\
                 (1204 earlier lines were dropped to keep the log small)\n\
                 14:21:07.412 INFO  resolve  3 streams\n\
                 14:21:08.902 INFO  player   first_frame_ms=1290\n\
                 \n\
                 ── Playback 1 of 2 · Episode 7 · 13:48 ──\n\
                 13:48:02.118 INFO  resolve  2 streams\n"
            )
        );
    }
}
