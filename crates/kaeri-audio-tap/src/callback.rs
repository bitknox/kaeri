//! The function libmpv calls. The one place in the crate that reads memory it was handed.
#![allow(unsafe_code)]

use std::ffi::{c_int, c_void};

use crate::{Tap, deliver};

/// libmpv's `mpv_audio_tap_fn`: `count` samples at `samples`, the first `pts` seconds into the
/// file tagged `tag`, or a break when `count` is 0. `ctx` is not used: the tag says whose audio
/// it is.
///
/// # Safety
///
/// `samples` points at `count` initialised samples for the length of the call, or is null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kaeri_audio_tap(
    _ctx: *mut c_void,
    tag: i64,
    pts: f64,
    samples: *const i16,
    count: c_int,
) {
    let count = usize::try_from(count).unwrap_or(0);
    if count == 0 || samples.is_null() || !pts.is_finite() {
        deliver(tag, Tap::Break);
        return;
    }
    // SAFETY: the caller hands over `count` initialised samples at `samples`, valid for the
    // call; they are copied before it returns, and null was ruled out above.
    let samples = unsafe { std::slice::from_raw_parts(samples, count) }.to_vec();
    deliver(tag, Tap::Samples { pts, samples });
}

/// The address of [`kaeri_audio_tap`], for an app to hand to `mpv_set_audio_tap` as an
/// `mpv_audio_tap_fn`.
pub fn callback_address() -> usize {
    kaeri_audio_tap as *const () as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{listen, next_tag};

    #[test]
    fn the_callback_copies_the_samples_and_reads_no_samples_as_a_break() {
        let tag = next_tag();
        let listener = listen(tag);
        let samples = [5i16, -5, 7];
        // SAFETY: three samples, as the count says; then null, which is a break.
        unsafe {
            kaeri_audio_tap(std::ptr::null_mut(), tag, 2.5, samples.as_ptr(), 3);
            kaeri_audio_tap(std::ptr::null_mut(), tag, 2.6, std::ptr::null(), 0);
            kaeri_audio_tap(std::ptr::null_mut(), tag, 2.7, std::ptr::null(), 4);
        }
        assert_eq!(
            listener.receiver().try_iter().collect::<Vec<_>>(),
            [Tap::Samples { pts: 2.5, samples: vec![5, -5, 7] }, Tap::Break, Tap::Break]
        );
        assert_ne!(callback_address(), 0);
    }
}
