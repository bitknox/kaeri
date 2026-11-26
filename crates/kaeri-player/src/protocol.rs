//! mpv's JSON IPC protocol, and how the mpv process is started.
//!
//! What mpv is told and what it says, and the options a file is opened with, are the same for
//! any mpv the engine drives, and are re-exported here. What is defined here belongs to the
//! child process alone: its command line, and the version its banner reports.

pub use kaeri_playback::protocol::*;

/// mpv's version, as far as the player's choices depend on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MpvVersion {
    pub major: u32,
    pub minor: u32,
}

impl MpvVersion {
    /// Read the version from the first line of `mpv --version`: `mpv v0.41.0 Copyright …` from
    /// release builds, `mpv 0.35.1 Copyright …` from Debian's, `mpv v0.41.0-112-gdeadbeef` from
    /// git builds.
    pub fn parse(banner: &str) -> Option<Self> {
        let version = banner.split_whitespace().nth(1)?.trim_start_matches('v');
        let mut parts = version.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor: String = parts.next()?.chars().take_while(char::is_ascii_digit).collect();
        Some(Self { major, minor: minor.parse().ok()? })
    }

    const fn at_least(self, major: u32, minor: u32) -> bool {
        self.major > major || (self.major == major && self.minor >= minor)
    }

    /// `loadfile` with an index argument before its options: 0.38.
    pub const fn has_loadfile_index(self) -> bool {
        self.at_least(0, 38)
    }

    /// The `user-data` property the companion is driven through, and `--osd-fonts-dir` for the
    /// bundled font: both 0.36. Without them the companion has nothing to read and mpv refuses
    /// the font option outright, so older players run without Kaeri's interface.
    pub const fn has_companion_support(self) -> bool {
        self.at_least(0, 36)
    }

    /// `--sub-border-style`, with `--sub-outline-size` and `--sub-outline-color` as the
    /// outline's names: 0.39. An older mpv refuses those as unknown options and does not start.
    pub const fn has_border_style(self) -> bool {
        self.at_least(0, 39)
    }
}

impl std::fmt::Display for MpvVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// The arguments Kaeri always passes to mpv.
///
/// `--idle=yes` with `--force-window=yes` keeps the window up between files, and that is what
/// lets one mpv carry a whole binge: the next episode, or a fallback stream, loads into the same
/// window with fullscreen and position intact, rather than a new process opening a new window
/// (on macOS, a fullscreen Space sliding away and back every episode). The playback loop decides
/// when the player is done; it is no longer inferred from mpv exiting at the end of a file.
pub fn base_args(socket_path: &str) -> Vec<String> {
    vec![
        format!("--input-ipc-server={socket_path}"),
        // Our own IPC must not be fought over by a user config that sets its own socket.
        "--no-input-terminal".to_string(),
        "--idle=yes".to_string(),
        "--force-window=yes".to_string(),
        // Keeps a partially-downloaded torrent stream from being treated as a hard error
        // when a read stalls waiting for pieces.
        "--network-timeout=60".to_string(),
    ]
}

/// `--clipboard-backends=vo` when the `mpv` on PATH has that option (it came with mpv's
/// clipboard, in 0.40), and nothing for an older one, which refuses an option it doesn't know
/// and doesn't start. Asked of `mpv --list-options` once.
///
/// mpv's clipboard connects to the Wayland compositor at start, on its main thread and before
/// it opens `--input-ipc-server`, so a compositor that is down or restarting holds the control
/// socket back for seconds. The `vo` backend asks only the window mpv opens itself.
pub fn vo_clipboard_args() -> &'static [String] {
    static ARGS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    ARGS.get_or_init(|| {
        let listed = std::process::Command::new("mpv")
            .args(["--no-config", "--list-options"])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
            .unwrap_or_default();
        if lists_option(&listed, "clipboard-backends") {
            vec!["--clipboard-backends=vo".to_owned()]
        } else {
            Vec::new()
        }
    })
}

/// Whether `mpv --list-options` output names `option` itself, not only a longer option that
/// starts with it.
fn lists_option(listed: &str, option: &str) -> bool {
    let wanted = format!("--{option}");
    listed.lines().any(|line| line.split_whitespace().next() == Some(wanted.as_str()))
}

/// Flags that belong to the player rather than to one file.
///
/// Speed and volume are set once when the window opens and then live in the session: a viewer
/// who slows playback down mid-episode keeps that speed into the next one because it is the same
/// player, not because anything was carried.
pub fn session_args(speed: Option<f64>, volume: Option<f64>, fullscreen: bool) -> Vec<String> {
    let mut args = Vec::new();
    if fullscreen {
        args.push("--fullscreen".to_string());
    }
    if let Some(speed) = speed.filter(|s| (*s - 1.0).abs() > f64::EPSILON) {
        args.push(format!("--speed={speed}"));
    }
    // Clamped to mpv's own 0..=100 baseline range; a remembered value beyond it would be
    // a corrupt config more often than an intent.
    if let Some(volume) = volume.filter(|v| (0.0..=100.0).contains(v)) {
        args.push(format!("--volume={volume}"));
    }
    args
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_outlives_the_file() {
        // One window per binge: without idle and a forced window, mpv closes between episodes
        // and the next one opens a new window, fullscreen lost.
        let args = base_args("/tmp/sock");
        assert!(args.iter().any(|a| a == "--idle=yes"));
        assert!(args.iter().any(|a| a == "--force-window=yes"));
        assert!(args.iter().any(|a| a == "--input-ipc-server=/tmp/sock"));
    }

    #[test]
    fn normal_speed_is_not_passed_but_a_carried_speed_is() {
        assert!(session_args(Some(1.0), None, false).is_empty());
        assert!(session_args(Some(1.5), None, false).iter().any(|a| a == "--speed=1.5"));
        assert!(session_args(None, None, true).iter().any(|a| a == "--fullscreen"));
    }

    #[test]
    fn the_clipboard_option_is_found_only_where_mpv_lists_it() {
        let newer = " --clipboard-backends             Object settings list (default: win32,mac,wayland,x11,vo)\n    --clipboard-backends-add\n --clipboard-monitor              Flag (default: no)\n";
        let older = " --cache                          Choices: no yes auto (default: auto)\n --clipboard-monitor-x            Flag (default: no)\n";
        assert!(lists_option(newer, "clipboard-backends"));
        assert!(!lists_option(older, "clipboard-backends"));
        assert!(!lists_option("", "clipboard-backends"), "no mpv, no option");
        assert!(!lists_option("    --clipboard-backends-add\n", "clipboard-backends"));
    }

    #[test]
    fn mpv_versions_read_from_every_banner_style() {
        let v = |s| MpvVersion::parse(s).unwrap();
        assert_eq!(v("mpv v0.41.0 Copyright © 2000-2025"), MpvVersion { major: 0, minor: 41 });
        assert_eq!(v("mpv 0.35.1 Copyright © 2000-2023"), MpvVersion { major: 0, minor: 35 });
        assert_eq!(
            v("mpv v0.41.0-112-gdeadbeef Copyright"),
            MpvVersion { major: 0, minor: 41 }
        );
        assert_eq!(v("mpv 0.40.0+git20250101 Copyright"), MpvVersion { major: 0, minor: 40 });
        assert!(MpvVersion::parse("not mpv").is_none());
        assert!(
            v("mpv 0.37.0").has_companion_support() && !v("mpv 0.37.0").has_loadfile_index()
        );
        assert!(!v("mpv 0.35.1").has_companion_support());
        assert!(v("mpv v0.38.0").has_loadfile_index());
    }
}
