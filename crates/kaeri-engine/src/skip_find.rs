//! Whether skip times are found from the audio now, and why not: Settings' "Find skip times from
//! the audio", the player's ability to hand the audio over, and the device's state. Finding
//! itself is the playback's, in `playback::session::found`.

use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

use kaeri_core::config::Config;
use kaeri_model::playback::SkipFindPause;

use crate::{outbox::Outbox, state::EngineState};

/// Under this much free storage finding pauses, in bytes: 2 GB, as Settings says.
pub const STORAGE_FLOOR: u64 = 2_000_000_000;

/// Why finding pauses, from the device's power saving mode and the storage free. Storage comes
/// first: it is the one the person can do something about from Settings' line.
pub fn paused(low_power: bool, free: Option<u64>) -> Option<SkipFindPause> {
    if free.is_some_and(|free| free < STORAGE_FLOOR) {
        Some(SkipFindPause::Storage)
    } else if low_power {
        Some(SkipFindPause::LowPower)
    } else {
        None
    }
}

/// What finding goes by, shared by the engine and every playback.
#[derive(Debug)]
pub struct SkipFind {
    /// Whether the player the front end hosts hands the audio over: the libmpv builds with the
    /// demuxer tap. mpv's own process never does.
    audio_tap: bool,
    /// Where the database is, whose disk the prints go on.
    data_dir: PathBuf,
    /// The storage free in place of asking the disk, for a test.
    free: Option<u64>,
    /// The device's power saving mode, as its front end last said.
    low_power: AtomicBool,
}

impl SkipFind {
    pub const fn new(audio_tap: bool, data_dir: PathBuf, free: Option<u64>) -> Self {
        Self { audio_tap, data_dir, free, low_power: AtomicBool::new(false) }
    }

    pub const fn audio_tap(&self) -> bool {
        self.audio_tap
    }

    pub fn set_low_power(&self, on: bool) {
        self.low_power.store(on, Ordering::Relaxed);
    }

    fn free(&self) -> Option<u64> {
        self.free.or_else(|| crate::downloads::ahead::free_space(&self.data_dir))
    }

    /// Why finding is paused while the setting is on, or `None` while it runs or is off. A
    /// player without the tap comes first: nothing else would let it run.
    pub fn paused(&self, config: &Config) -> Option<SkipFindPause> {
        if !config.playback.find_skip_times {
            return None;
        }
        if !self.audio_tap {
            return Some(SkipFindPause::Unavailable);
        }
        paused(self.low_power.load(Ordering::Relaxed), self.free())
    }

    /// Whether a playback starting now finds skip times.
    pub fn runs(&self, config: &Config) -> bool {
        config.playback.find_skip_times && self.paused(config).is_none()
    }
}

impl Default for SkipFind {
    fn default() -> Self {
        Self::new(false, PathBuf::new(), None)
    }
}

/// Say Settings again when why finding is paused has changed: the device's power saving mode,
/// the storage free or the setting itself.
pub fn refresh(find: &SkipFind, engine: &mut EngineState, tx: &Outbox) {
    let now = find.paused(engine.config.effective());
    if engine.config.skip_find_paused() != now {
        engine.config.set_skip_find_paused(now);
        let _ = tx.send(kaeri_model::event::Event::Settings(engine.config.view()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_comes_before_power_and_unknown_storage_is_room() {
        assert_eq!(paused(false, None), None);
        assert_eq!(paused(false, Some(STORAGE_FLOOR)), None);
        assert_eq!(paused(true, Some(STORAGE_FLOOR)), Some(SkipFindPause::LowPower));
        assert_eq!(paused(true, Some(STORAGE_FLOOR - 1)), Some(SkipFindPause::Storage));
        assert_eq!(paused(false, Some(1)), Some(SkipFindPause::Storage));
    }

    #[test]
    fn it_runs_only_with_the_tap_the_setting_and_no_pause() {
        let config = Config::default();
        assert!(config.playback.find_skip_times, "on by default");
        let find = SkipFind::new(true, PathBuf::new(), Some(STORAGE_FLOOR * 2));
        assert!(find.runs(&config));
        find.set_low_power(true);
        assert!(!find.runs(&config));
        assert_eq!(find.paused(&config), Some(SkipFindPause::LowPower));
        let mut off = config.clone();
        off.playback.find_skip_times = false;
        assert_eq!(find.paused(&off), None, "nothing to say while it is off");
        find.set_low_power(false);
        assert!(!SkipFind::new(false, PathBuf::new(), None).runs(&config), "no tap");
    }

    /// A libmpv without the demuxer tap: finding never runs, Settings says why, and the reason
    /// stays put through the device's states. With the setting off there is nothing to say.
    #[test]
    fn without_the_tap_it_says_so_and_never_runs() {
        let config = Config::default();
        let find = SkipFind::new(false, PathBuf::new(), Some(STORAGE_FLOOR * 2));
        assert_eq!(find.paused(&config), Some(SkipFindPause::Unavailable));
        assert!(!find.runs(&config));
        find.set_low_power(true);
        assert_eq!(
            find.paused(&config),
            Some(SkipFindPause::Unavailable),
            "before power saving"
        );
        let low = SkipFind::new(false, PathBuf::new(), Some(1));
        assert_eq!(low.paused(&config), Some(SkipFindPause::Unavailable), "before storage");
        let mut off = config.clone();
        off.playback.find_skip_times = false;
        assert_eq!(find.paused(&off), None);
        assert!(!find.runs(&off));
    }
}
