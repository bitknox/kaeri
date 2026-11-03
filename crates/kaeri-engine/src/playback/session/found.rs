//! Finding a file's opening and ending from its audio, while it plays.
//!
//! The player's tap hands over the head and the tail of the file, ahead of the playhead, with
//! the tag this capture listens for. A thread of its own fingerprints them as they come
//! (`kaeri_playback::detect`), matches each part against the same part of the show's last two
//! episodes as soon as there is enough of it, and says each stretch it finds. When the file
//! closes the prints are kept for the next episodes, and what was found is kept for this one.
//! Nothing here leaves the device.

use std::{
    sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel},
    time::Duration,
};

use kaeri_audio_tap::{Listener, Tap};
use kaeri_core::ids::AnilistId;
use kaeri_playback::{
    SkipInterval,
    detect::{self, Capture, Part, Print},
};
use kaeri_store::{FoundSkip, SkipPrint, Store, skip_found::state};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

/// How many earlier episodes a part is matched against.
const PRIORS: usize = 2;
/// A part is matched again after this much more of its audio, in seconds.
const MATCH_EVERY: f64 = 8.0;
/// A part keeps its print only with at least this much heard, in seconds.
const KEEP_HEARD: f64 = 30.0;
/// Episodes whose found starts are within this of each other agree, in seconds.
const AGREE: f64 = 2.0;

/// A stretch found in the file playing.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct FoundTime {
    pub interval: SkipInterval,
}

enum Control {
    /// The file's length, which places the tail.
    Length(f64),
}

/// One file's capture, from its load to its end.
pub(super) struct Finder {
    tag: i64,
    control: Sender<Control>,
    found: UnboundedReceiver<FoundTime>,
    /// Whether the thread may still say something.
    open: bool,
    length: Option<f64>,
}

impl Finder {
    /// Listen for the tap's audio for one episode of `show`, before the file is loaded so that
    /// none of it is missed.
    pub(super) fn start(store: Store, show: AnilistId, episode: String) -> Self {
        let tag = kaeri_audio_tap::next_tag();
        let listener = kaeri_audio_tap::listen(tag);
        let (control, controlled) = channel();
        let (says, found) = unbounded_channel();
        let spawned = std::thread::Builder::new().name("skip-find".into()).spawn(move || {
            run(&listener, &controlled, &says, &store, show, &episode);
        });
        if let Err(error) = spawned {
            tracing::warn!(%error, "no skip finding for this file");
        }
        Self { tag, control, found, open: true, length: None }
    }

    /// The load's options that turn the tap on for this file: the head until the length is
    /// known.
    pub(super) fn options(&self) -> Vec<(String, String)> {
        vec![
            ("demuxer-audio-tap-tag".into(), self.tag.to_string()),
            ("demuxer-audio-tap-windows".into(), Part::tap_windows(None)),
        ]
    }

    /// The file's length is known: the tail's window, to set on the player, the first time.
    pub(super) fn length(&mut self, length: f64) -> Option<String> {
        if self.length.is_some() || length <= 0.0 {
            return None;
        }
        self.length = Some(length);
        let _ = self.control.send(Control::Length(length));
        Some(Part::tap_windows(Some(length)))
    }

    pub(super) const fn is_open(&self) -> bool {
        self.open
    }

    /// The next stretch found, or `None` once the capture has nothing more to say.
    pub(super) async fn next(&mut self) -> Option<FoundTime> {
        let found = self.found.recv().await;
        if found.is_none() {
            self.open = false;
        }
        found
    }
}

/// The show's last prints of `part`, but this episode's.
fn priors(store: &Store, show: AnilistId, episode: &str, part: Part) -> Vec<(String, Print)> {
    store
        .skip_prints(show, part.as_str())
        .inspect_err(|error| tracing::debug!(%error, "no earlier prints"))
        .unwrap_or_default()
        .into_iter()
        .filter(|print| print.episode != episode)
        .take(PRIORS)
        .map(|print| {
            let start = ms_to_secs(print.start_ms);
            (print.episode, Print::from_bytes(start, &print.items))
        })
        .collect()
}

#[expect(clippy::cast_precision_loss, reason = "milliseconds within an episode")]
fn ms_to_secs(ms: i64) -> f64 {
    ms as f64 / 1000.0
}

#[expect(clippy::cast_possible_truncation, reason = "seconds within an episode")]
fn secs_to_ms(secs: f64) -> i64 {
    (secs * 1000.0).round() as i64
}

/// One part's state on the thread.
struct Matching {
    part: Part,
    priors: Vec<(String, Print)>,
    /// Seconds of audio since the last match.
    fresh: f64,
    found: bool,
}

fn run(
    listener: &Listener,
    control: &Receiver<Control>,
    says: &UnboundedSender<FoundTime>,
    store: &Store,
    show: AnilistId,
    episode: &str,
) {
    let mut capture = Capture::new();
    let mut parts: Vec<Matching> = Part::ALL
        .into_iter()
        .map(|part| Matching {
            part,
            priors: priors(store, show, episode, part),
            fresh: 0.0,
            found: false,
        })
        .collect();
    let found = |capture: &Capture, matching: &mut Matching, settled: bool| {
        if matching.found {
            return;
        }
        let Some(this) = capture.print(matching.part) else { return };
        let mut hits = matching.priors.iter().filter_map(|(other, print)| {
            detect::find(matching.part, this, print, settled).map(|found| (other, found))
        });
        let Some((first, found)) = hits.next() else { return };
        let mut agreed = vec![first.clone()];
        agreed.extend(
            hits.filter(|(_, other)| (other.start - found.start).abs() <= AGREE)
                .map(|(episode, _)| episode.clone()),
        );
        matching.found = true;
        let length = capture.length().unwrap_or(0.0);
        let row = FoundSkip {
            anilist_id: show,
            episode: episode.to_owned(),
            kind: found.kind.api_type().to_owned(),
            start_ms: secs_to_ms(found.start),
            end_ms: secs_to_ms(found.end),
            length_ms: secs_to_ms(length),
            agreed: agreed.join(","),
            state: state::FOUND.to_owned(),
            found_at: kaeri_store::now(),
        };
        if let Err(error) = store.put_found_skip(&row) {
            tracing::debug!(%error, "found skip time not kept");
        }
        tracing::info!(kind = ?found.kind, start = found.start, end = found.end, "skip time found");
        let _ = says.send(FoundTime { interval: found.interval() });
    };

    loop {
        while let Ok(Control::Length(length)) = control.try_recv() {
            capture.set_length(length);
        }
        match listener.receiver().recv_timeout(Duration::from_millis(250)) {
            Ok(Tap::Samples { pts, samples }) => {
                capture.samples(pts, &samples);
                let tail = capture.print(Part::Tail).is_some_and(|tail| pts >= tail.start);
                let part = if tail { 1 } else { 0 };
                parts[part].fresh += samples.len() as f64 / f64::from(detect::RATE);
                if parts[part].fresh >= MATCH_EVERY {
                    parts[part].fresh = 0.0;
                    found(&capture, &mut parts[part], false);
                }
            }
            Ok(Tap::Break) => {
                capture.brk();
                for matching in &mut parts {
                    found(&capture, matching, false);
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                // The finder is gone: the file has closed.
                if says.is_closed() {
                    break;
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    capture.brk();
    for matching in &mut parts {
        found(&capture, matching, true);
    }
    let length = capture.length();
    for part in Part::ALL {
        let Some(print) = capture.print(part) else { continue };
        if print.heard() < KEEP_HEARD {
            continue;
        }
        let row = SkipPrint {
            anilist_id: show,
            episode: episode.to_owned(),
            part: part.as_str().to_owned(),
            length_ms: secs_to_ms(length.unwrap_or(0.0)),
            start_ms: secs_to_ms(print.start),
            items: print.to_bytes(),
            taken_at: kaeri_store::now(),
        };
        if let Err(error) = store.put_skip_print(&row) {
            tracing::debug!(%error, "print not kept");
        }
    }
}
