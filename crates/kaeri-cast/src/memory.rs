//! What the engine knows of each TV.
//!
//! What a TV played and refused lasts for one cast session: a refusal is only ever a reason to
//! change one part of the next plan in the same session, and the next connect tries the stream
//! as it is again. What is kept across launches, in `cast-tv-notes.json` in the engine's data
//! directory, is small: the TV's name, model, firmware and volume control, the route of its last
//! cast for the picker's hint, and whether the viewer heard about its plain subtitles. Whether
//! Kaeri's receiver failed on a TV is never kept: every cast tries it first. A file written when
//! that was kept still reads, and the old field is dropped at the next write. The file is written
//! whole to a temporary file and renamed over the old one. `cast-tvs.json`, which kept refusals
//! for 30 days, is deleted when the memory is first opened.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
};

use serde::{Deserialize, Serialize};

use crate::{client::VolumeControl, formats::CastPlan};

/// Where the memory is kept, in the engine's data directory.
pub const FILE: &str = "cast-tv-notes.json";
/// The file that kept refusals for 30 days, deleted on open.
const OLD_FILE: &str = "cast-tvs.json";

/// What happened when a TV was handed one format part, and when.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormatAnswer {
    pub played: bool,
    /// Unix seconds.
    pub at: i64,
}

/// What the engine has learned about one TV.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CastCaps {
    /// The TXT `id`.
    pub id: String,
    pub name: String,
    pub model: String,
    /// `cast_build_revision` from 8008 `/setup/eureka_info`, when the TV answers it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firmware: Option<String>,
    #[serde(default)]
    pub volume: VolumeControl,
    /// Each format part (`hevc-main10`, `opus`, `mkv`) with what happened and when, in this
    /// cast session: never written down.
    #[serde(skip)]
    pub formats: BTreeMap<String, FormatAnswer>,
    /// The last cast to it went through this device: the picker's hint for the next.
    #[serde(default)]
    pub relayed_last: bool,
    /// The viewer was told that Kaeri's receiver shows plain subtitles on it: once per TV.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub told_subtitles_plain: bool,
}

impl CastCaps {
    /// What is known of a TV never cast to.
    pub fn blank(id: &str) -> Self {
        Self { id: id.to_owned(), ..Self::default() }
    }

    /// Whether the TV refused `part` in this session.
    pub fn refused(&self, part: &str) -> bool {
        self.formats.get(part).is_some_and(|answer| !answer.played)
    }

    pub fn played(&self, part: &str) -> bool {
        self.formats.get(part).is_some_and(|answer| answer.played)
    }

    /// Note what happened to `part` at `at`.
    pub fn remember(&mut self, part: &str, played: bool, at: i64) {
        self.formats.insert(part.to_owned(), FormatAnswer { played, at });
    }

    /// Note what a plan did, when the receiver didn't say which part failed. Played: every
    /// part of it played. Refused: the blame goes to one part not already known to play, the
    /// copied audio first, then the picture, then the container, so an audio codec never counts
    /// against the video. When
    /// every part is known to play, nothing is marked: it was the stream, not the format.
    pub fn record(&mut self, plan: &CastPlan, played: bool, at: i64) {
        let parts = plan.parts();
        if played {
            for part in parts {
                self.remember(&part, true, at);
            }
        } else {
            // The copied audio, then the picture, then the container.
            let audio = plan.audio_part();
            let blamed = parts
                .iter()
                .filter(|part| Some(*part) == audio.as_ref())
                .chain(parts.iter().filter(|part| Some(*part) != audio.as_ref()))
                .find(|part| !self.played(part));
            if let Some(part) = blamed {
                self.remember(&part.clone(), false, at);
            }
        }
        self.relayed_last = plan.relayed();
    }

    /// Note a plan the TV took and never played, with every byte it asked for served in good
    /// time. That says less than a refusal: the Nest Hub sits in `BUFFERING` on audio it can't
    /// decode, but a picture or a container it can't read is refused outright. So only a copied
    /// audio codec not known to play is blamed, and a plan without one teaches nothing. Returns
    /// the part blamed.
    pub fn record_stall(&mut self, plan: &CastPlan, at: i64) -> Option<String> {
        let part = plan.audio_part().filter(|part| !self.played(part))?;
        self.remember(&part, false, at);
        Some(part)
    }
}

/// The clock the memory reads, so a test can move it.
pub type Clock = Arc<dyn Fn() -> i64 + Send + Sync>;

fn system_clock() -> Clock {
    Arc::new(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
    })
}

#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
}

/// Every TV's [`CastCaps`].
pub struct CastMemory {
    path: Option<PathBuf>,
    tvs: Mutex<BTreeMap<String, CastCaps>>,
    clock: Clock,
}

impl std::fmt::Debug for CastMemory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CastMemory").field("path", &self.path).finish_non_exhaustive()
    }
}

impl CastMemory {
    /// Kept in memory only, for tests and for a build without a data directory.
    pub fn memory() -> Self {
        Self { path: None, tvs: Mutex::default(), clock: system_clock() }
    }

    /// The memory in `dir`. A file that can't be read is set aside and started afresh: what a
    /// TV plays is learnt again in a cast or two, and a cast should never fail over it.
    pub fn open(dir: &Path) -> Result<Self, MemoryError> {
        // Refusals kept for 30 days, which could refuse a stream the TV plays.
        let old = dir.join(OLD_FILE);
        match std::fs::remove_file(&old) {
            Ok(()) => tracing::info!("deleted the cast refusals of before"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => tracing::warn!(error = %e, "couldn't delete {OLD_FILE}"),
        }
        let path = dir.join(FILE);
        let mut tvs = BTreeMap::new();
        match std::fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<Vec<CastCaps>>(&bytes) {
                Ok(list) => tvs.extend(list.into_iter().map(|tv| (tv.id.clone(), tv))),
                Err(e) => {
                    tracing::warn!(error = %e, "{FILE} unreadable; starting afresh");
                    let _ = std::fs::rename(&path, path.with_extension("json.unreadable"));
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        Ok(Self { path: Some(path), tvs: Mutex::new(tvs), clock: system_clock() })
    }

    /// The same memory, read by `clock` instead of the system's.
    #[must_use]
    pub fn with_clock(mut self, clock: Clock) -> Self {
        self.clock = clock;
        self
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, CastCaps>> {
        self.tvs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A cast session to the TV `id` begins: what it played and refused in the last one is
    /// forgotten, so the stream is tried as it is again.
    pub fn new_session(&self, id: &str) {
        if let Some(caps) = self.lock().get_mut(id) {
            caps.formats.clear();
        }
    }

    /// What is known of the TV `id`; a blank record for a TV never cast to.
    pub fn caps(&self, id: &str) -> CastCaps {
        self.lock().get(id).cloned().unwrap_or_else(|| CastCaps::blank(id))
    }

    /// The TV `id` was reached, calling itself `name`, a `model`, with `firmware` when it said,
    /// and its volume works as `volume`.
    pub fn seen(
        &self,
        id: &str,
        name: &str,
        model: &str,
        firmware: Option<&str>,
        volume: VolumeControl,
    ) -> Result<(), MemoryError> {
        {
            let mut tvs = self.lock();
            let caps = tvs.entry(id.to_owned()).or_insert_with(|| CastCaps::blank(id));
            if let Some(firmware) = firmware {
                caps.firmware = Some(firmware.to_owned());
            }
            name.clone_into(&mut caps.name);
            model.clone_into(&mut caps.model);
            caps.volume = volume;
        }
        self.save()
    }

    /// Note what `plan` did on the TV `id`.
    pub fn record(&self, id: &str, plan: &CastPlan, played: bool) -> Result<(), MemoryError> {
        let now = (self.clock)();
        self.lock()
            .entry(id.to_owned())
            .or_insert_with(|| CastCaps::blank(id))
            .record(plan, played, now);
        self.save()
    }

    /// Note that `plan` stalled on the TV `id`, as [`CastCaps::record_stall`] says.
    pub fn record_stall(
        &self,
        id: &str,
        plan: &CastPlan,
    ) -> Result<Option<String>, MemoryError> {
        let now = (self.clock)();
        let blamed = self
            .lock()
            .entry(id.to_owned())
            .or_insert_with(|| CastCaps::blank(id))
            .record_stall(plan, now);
        if blamed.is_some() {
            self.save()?;
        }
        Ok(blamed)
    }

    /// Note that the viewer heard TV `id` shows subtitles as plain text. `true` the first time.
    pub fn told_subtitles_plain(&self, id: &str) -> Result<bool, MemoryError> {
        let first = {
            let mut all = self.lock();
            let caps = all.entry(id.to_owned()).or_insert_with(|| CastCaps::blank(id));
            !std::mem::replace(&mut caps.told_subtitles_plain, true)
        };
        if first {
            self.save()?;
        }
        Ok(first)
    }

    /// Note what TV `id` did with one part alone: a capability clip's answer.
    pub fn note(&self, id: &str, part: &str, played: bool) -> Result<(), MemoryError> {
        let now = (self.clock)();
        self.lock()
            .entry(id.to_owned())
            .or_insert_with(|| CastCaps::blank(id))
            .remember(part, played, now);
        self.save()
    }

    fn save(&self) -> Result<(), MemoryError> {
        let Some(path) = &self.path else { return Ok(()) };
        let list: Vec<CastCaps> = self.lock().values().cloned().collect();
        let body = serde_json::to_vec_pretty(&list)?;
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, body)?;
        std::fs::rename(&temp, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::{AudioCodec, AudioPlan, CastRoute, Delivery, VideoFormat};

    fn plan(video: VideoFormat, audio: AudioCodec, delivery: Delivery) -> CastPlan {
        CastPlan {
            route: CastRoute::Relayed { remux: true },
            video,
            height: None,
            audio: AudioPlan::Copy(audio),
            delivery,
        }
    }

    #[test]
    fn a_refusal_blames_the_first_part_not_known_to_play() {
        let mut caps = CastCaps::default();
        caps.record(&plan(VideoFormat::Hevc, AudioCodec::Aac, Delivery::HlsFmp4), true, 1);
        caps.record(&plan(VideoFormat::Hevc, AudioCodec::Opus, Delivery::HlsFmp4), false, 2);
        assert!(caps.played("hevc/1080/hls"));
        assert!(caps.refused("opus/hls"));
        assert!(!caps.refused("hls-fmp4"));
        assert!(caps.relayed_last);

        // Every part known to play: the stream's fault, not the format's.
        let before = caps.formats.clone();
        caps.record(&plan(VideoFormat::Hevc, AudioCodec::Aac, Delivery::HlsFmp4), false, 3);
        assert_eq!(caps.formats, before);
    }

    #[test]
    fn a_stall_blames_copied_audio_and_never_the_picture_or_the_container() {
        let mut caps = CastCaps::default();
        let mut tall = plan(VideoFormat::H264, AudioCodec::Aac, Delivery::HlsFmp4);
        tall.height = Some(1080);
        assert_eq!(caps.record_stall(&tall, 1), None);
        let mut mkv = plan(VideoFormat::Hevc, AudioCodec::Aac, Delivery::Mkv);
        mkv.route = CastRoute::Relayed { remux: false };
        assert_eq!(caps.record_stall(&mkv, 1), None);
        assert!(caps.formats.is_empty(), "{:?}", caps.formats);

        let flac = plan(VideoFormat::H264, AudioCodec::Flac, Delivery::HlsFmp4);
        assert_eq!(caps.record_stall(&flac, 2).as_deref(), Some("flac/hls"));
        assert!(caps.refused("flac/hls"));
        assert_eq!(caps.formats.len(), 1);
    }

    #[test]
    fn a_failure_of_kaeris_receiver_kept_by_an_older_version_is_read_and_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let old = r#"[{"id":"tv","name":"Living room","model":"Chromecast","firmware":"1.0",
            "volume":"level","own_receiver_failed":1000,"relayed_last":true}]"#;
        std::fs::write(dir.path().join(FILE), old).unwrap();
        let memory = CastMemory::open(dir.path()).unwrap();
        let caps = memory.caps("tv");
        assert_eq!(caps.name, "Living room");
        assert!(caps.relayed_last);
        assert!(!dir.path().join(FILE).with_extension("json.unreadable").exists());
        memory
            .seen("tv", "Living room", "Chromecast", Some("1.0"), VolumeControl::Level)
            .unwrap();
        let written = std::fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert!(!written.contains("own_receiver_failed"), "{written}");
    }

    #[test]
    fn a_refusal_never_outlives_the_session() {
        let dir = tempfile::tempdir().unwrap();
        let memory = CastMemory::open(dir.path()).unwrap();
        memory
            .seen("tv", "Living room", "Chromecast", Some("1.0"), VolumeControl::Level)
            .unwrap();
        let hevc = plan(VideoFormat::HevcMain10, AudioCodec::Aac, Delivery::HlsFmp4);
        memory.record("tv", &hevc, false).unwrap();
        let refused = |caps: &CastCaps| caps.formats.values().any(|answer| !answer.played);
        assert!(refused(&memory.caps("tv")), "heeded in the session");

        // Another launch reads none of it.
        let again = CastMemory::open(dir.path()).unwrap();
        let caps = again.caps("tv");
        assert!(caps.formats.is_empty(), "{:?}", caps.formats);
        assert_eq!(caps.name, "Living room");
        let written = std::fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert!(!written.contains("hevc"), "{written}");

        // A second connect in this launch tries it again.
        memory.new_session("tv");
        assert!(!refused(&memory.caps("tv")));
    }

    #[test]
    fn an_audio_refusal_never_blames_the_video() {
        let mut caps = CastCaps::default();
        let mut tall = plan(VideoFormat::H264, AudioCodec::Opus, Delivery::HlsFmp4);
        tall.height = Some(1080);
        caps.record(&tall, false, 1);
        assert!(caps.refused("opus/hls"));
        assert!(!caps.refused("h264/1080/hls"), "{:?}", caps.formats);
    }

    #[test]
    fn the_refusals_of_before_are_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let old = r#"[{"id":"tv","name":"Bedroom","model":"Chromecast","judgement":2,
            "formats":{"h264/1080/hls":{"played":false,"at":1}}}]"#;
        std::fs::write(dir.path().join(OLD_FILE), old).unwrap();
        let memory = CastMemory::open(dir.path()).unwrap();
        assert!(!dir.path().join(OLD_FILE).exists());
        assert!(!memory.caps("tv").refused("h264/1080/hls"));
    }

    #[test]
    fn an_unreadable_file_is_set_aside() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE), b"{not json").unwrap();
        let memory = CastMemory::open(dir.path()).unwrap();
        assert!(memory.caps("tv").formats.is_empty());
        assert!(dir.path().join(FILE).with_extension("json.unreadable").exists());
    }
}
