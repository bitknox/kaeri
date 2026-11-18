//! Which stream a playback moves to when the one playing fails: before its first frame, when it
//! breaks off mid-episode, or when the viewer takes the stall prompt's offer.
//!
//! The stream that takes over should look and sound like the one that failed. The pool is
//! ordered best first by resolution (the quality setting, then each server's health), and the
//! pick keeps that order within each step: the same quality from the same source first, then
//! the same quality from another source, then the next quality down, and a higher one only when
//! nothing else is left, so a failover never costs more data than the stream it replaces unless
//! it has to.

use std::cmp::Reverse;

use kaeri_core::stream::Stream;

/// How near the end a stream that breaks off is let go rather than replaced: the credits and
/// the preview, which are not worth a new stream and a search for its place.
pub(super) const FAILOVER_TAIL: f64 = 120.0;

/// The stream to move to from `current`, one not yet `tried`, as like it as the pool allows.
/// `target` is the quality the viewer picked by hand from the sources menu, which later
/// failovers keep to; `None` keeps to the quality of the stream that failed. `None` when
/// every other stream has been tried.
pub(super) fn next_like(
    streams: &[Stream],
    tried: &[bool],
    current: usize,
    target: Option<u32>,
) -> Option<usize> {
    let failing = streams.get(current)?;
    let target = target.or(failing.quality);
    (0..streams.len())
        .filter(|&i| i != current && !tried.get(i).copied().unwrap_or(false))
        .min_by_key(|&i| {
            let stream = &streams[i];
            let step = Step::of(target, stream.quality);
            let other_source = stream.provider_id != failing.provider_id;
            (step, other_source, i)
        })
}

/// How far a stream's quality is from the one wanted, in the order they are tried.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Step {
    /// The quality wanted, or any when the quality is not known on either side.
    Same,
    /// Lower, the nearest first.
    Lower(Reverse<u32>),
    /// Not known, where the one wanted is: tried before anything known to be higher.
    Unknown,
    /// Higher, the nearest first.
    Higher(u32),
}

impl Step {
    fn of(target: Option<u32>, quality: Option<u32>) -> Self {
        match (target, quality) {
            (None, _) => Self::Same,
            (Some(_), None) => Self::Unknown,
            (Some(want), Some(have)) if have == want => Self::Same,
            (Some(want), Some(have)) if have < want => Self::Lower(Reverse(have)),
            (Some(_), Some(have)) => Self::Higher(have),
        }
    }
}

#[cfg(test)]
mod tests {
    use kaeri_core::stream::StreamKind;

    use super::*;

    fn stream(source: &str, quality: Option<u32>) -> Stream {
        let url = format!("https://{source}.example/stardust-wayfarer/1.m3u8");
        Stream { provider_id: source.into(), quality, ..Stream::new(url, StreamKind::Hls) }
    }

    fn pool(streams: &[(&str, Option<u32>)]) -> Vec<Stream> {
        streams.iter().map(|(source, quality)| stream(source, *quality)).collect()
    }

    /// The order a pool is gone through from `current`, each pick tried in turn.
    fn walk(streams: &[Stream], current: usize, target: Option<u32>) -> Vec<usize> {
        let mut tried = vec![false; streams.len()];
        tried[current] = true;
        let mut order = Vec::new();
        let mut at = current;
        while let Some(next) = next_like(streams, &tried, at, target) {
            tried[next] = true;
            order.push(next);
            at = next;
        }
        order
    }

    #[test]
    fn the_same_quality_from_the_same_source_comes_first_then_from_another() {
        let streams = pool(&[
            ("alpha", Some(1080)),
            ("beta", Some(1080)),
            ("alpha", Some(720)),
            ("alpha", Some(1080)),
        ]);
        let tried = [true, false, false, false];
        assert_eq!(next_like(&streams, &tried, 0, None), Some(3));
        let tried = [true, false, false, true];
        assert_eq!(next_like(&streams, &tried, 0, None), Some(1));
    }

    #[test]
    fn a_lower_quality_comes_before_a_higher_one_and_the_nearest_first() {
        let streams = pool(&[
            ("alpha", Some(720)),
            ("alpha", Some(1080)),
            ("beta", Some(360)),
            ("beta", Some(480)),
            ("gamma", None),
            ("alpha", Some(2160)),
        ]);
        // 480 before 360, an unknown quality before anything higher, then 1080 before 2160.
        assert_eq!(walk(&streams, 0, None), [3, 2, 4, 1, 5]);
    }

    #[test]
    fn a_stream_picked_by_hand_sets_the_quality_later_failovers_keep_to() {
        // The viewer picked the 720p stream by hand; it fails, and the other 720p is next,
        // ahead of the 1080p streams the pool ranks first.
        let streams = pool(&[
            ("alpha", Some(1080)),
            ("beta", Some(1080)),
            ("alpha", Some(720)),
            ("beta", Some(720)),
        ]);
        let tried = [false, false, true, false];
        assert_eq!(next_like(&streams, &tried, 2, Some(720)), Some(3));
        // And when the 720p from the other source fails too, the target still says 720p:
        // nothing lower is left, so the nearest higher, from the source that just failed.
        let tried = [false, false, true, true];
        assert_eq!(next_like(&streams, &tried, 3, Some(720)), Some(1));
    }

    #[test]
    fn with_no_quality_known_the_pool_order_stands_within_the_source() {
        let streams = pool(&[("alpha", None), ("beta", None), ("alpha", None)]);
        assert_eq!(walk(&streams, 0, None), [2, 1]);
    }

    #[test]
    fn a_pool_of_one_or_one_fully_tried_has_nothing_to_move_to() {
        let streams = pool(&[("alpha", Some(1080))]);
        assert_eq!(next_like(&streams, &[true], 0, None), None);
        let streams = pool(&[("alpha", Some(1080)), ("beta", Some(1080))]);
        assert_eq!(next_like(&streams, &[true, true], 0, None), None);
    }
}
