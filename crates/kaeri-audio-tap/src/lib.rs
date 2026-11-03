//! The audio libmpv's demuxer tap hands over (the patch in `android/mpv/patches/mpv`), for
//! found skip times.
//!
//! The app gives libmpv one C function, [`kaeri_audio_tap`], through `mpv_set_audio_tap`, once
//! per mpv core. The tap calls it on a thread of its own with 16-bit mono samples at 11025 Hz and
//! their playback-timeline timestamps, and with no samples for a break. Every call carries the
//! tag the engine set with `demuxer-audio-tap-tag` when it loaded the file, and the samples go to
//! whichever [`Listener`] holds that tag: the engine's capture for that file. Audio for a tag
//! nobody listens to is dropped.
//!
//! The callback never waits: when a listener falls behind, the audio it missed becomes a break.

use std::{
    collections::HashMap,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicI64, Ordering},
        mpsc::{Receiver, SyncSender, TrySendError, sync_channel},
    },
};

mod callback;

pub use callback::{callback_address, kaeri_audio_tap};

/// What the tap says.
#[derive(Debug, Clone, PartialEq)]
pub enum Tap {
    /// Samples at 11025 Hz, mono, the first `pts` seconds into the file.
    Samples { pts: f64, samples: Vec<i16> },
    /// The audio does not go on from the last samples.
    Break,
}

/// How many calls a listener may fall behind by before the audio becomes a break: about a
/// minute of audio in the tap's usual chunks.
const QUEUE: usize = 256;

struct Entry {
    tx: SyncSender<Tap>,
    /// Audio was dropped since the last call that got through.
    lost: bool,
}

fn listeners() -> &'static Mutex<HashMap<i64, Entry>> {
    static LISTENERS: OnceLock<Mutex<HashMap<i64, Entry>>> = OnceLock::new();
    LISTENERS.get_or_init(Default::default)
}

/// A tag no listener has had, for the next file.
pub fn next_tag() -> i64 {
    static NEXT: AtomicI64 = AtomicI64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// The audio tagged `tag`, until it is dropped.
pub struct Listener {
    tag: i64,
    rx: Receiver<Tap>,
}

impl Listener {
    pub const fn tag(&self) -> i64 {
        self.tag
    }

    pub const fn receiver(&self) -> &Receiver<Tap> {
        &self.rx
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        if let Ok(mut map) = listeners().lock() {
            map.remove(&self.tag);
        }
    }
}

/// Listen for the audio tagged `tag`. A second listener for a tag takes it over.
pub fn listen(tag: i64) -> Listener {
    let (tx, rx) = sync_channel(QUEUE);
    if let Ok(mut map) = listeners().lock() {
        map.insert(tag, Entry { tx, lost: false });
    }
    Listener { tag, rx }
}

/// Hand `tap` to the listener for `tag`, if any. What the callback does once it has copied the
/// samples, and what a host that taps the audio another way calls.
pub fn deliver(tag: i64, tap: Tap) {
    let Ok(mut map) = listeners().lock() else { return };
    let Some(entry) = map.get_mut(&tag) else { return };
    if entry.lost {
        match entry.tx.try_send(Tap::Break) {
            Ok(()) => entry.lost = false,
            Err(TrySendError::Full(_)) => return,
            Err(TrySendError::Disconnected(_)) => {
                map.remove(&tag);
                return;
            }
        }
    }
    match entry.tx.try_send(tap) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) => entry.lost = true,
        Err(TrySendError::Disconnected(_)) => {
            map.remove(&tag);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_goes_to_the_listener_for_its_tag_and_a_backlog_becomes_a_break() {
        let tag = next_tag();
        let listener = listen(tag);
        deliver(tag + 1_000_000, Tap::Break);
        deliver(tag, Tap::Samples { pts: 1.0, samples: vec![1, 2] });
        assert_eq!(
            listener.receiver().try_recv(),
            Ok(Tap::Samples { pts: 1.0, samples: vec![1, 2] })
        );
        assert!(listener.receiver().try_recv().is_err(), "not another tag's");

        for k in 0..QUEUE + 5 {
            deliver(tag, Tap::Samples { pts: k as f64, samples: vec![0] });
        }
        let got: Vec<Tap> = listener.receiver().try_iter().collect();
        assert_eq!(got.len(), QUEUE);
        deliver(tag, Tap::Samples { pts: 900.0, samples: vec![3] });
        assert_eq!(listener.receiver().try_recv(), Ok(Tap::Break), "what was lost is a break");
        assert_eq!(
            listener.receiver().try_recv(),
            Ok(Tap::Samples { pts: 900.0, samples: vec![3] })
        );

        drop(listener);
        assert!(listeners().lock().unwrap().get(&tag).is_none());
    }
}
