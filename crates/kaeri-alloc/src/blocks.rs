//! The large blocks: mapped from the system, kept when freed, unmapped on [`release`]. The one
//! place in the crate that handles raw memory.
#![allow(unsafe_code)]

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::UnsafeCell,
    ffi::{c_int, c_void},
    sync::atomic::{AtomicBool, Ordering},
};

use crate::LARGE;

/// The global allocator: blocks of [`LARGE`] bytes and over from the system's pages, kept for
/// reuse when freed until [`crate::release`]; everything else from [`System`].
pub struct Blocks;

/// The alignment a mapping always has, on every Apple platform.
const MAPPED_ALIGN: usize = 4096;

/// How many freed blocks are kept at once. Far more than the reader has in flight; a block freed
/// while every slot is taken goes back to the system at once.
const SLOTS: usize = 64;

/// Whether a block is one of ours: large, and aligned no further than a mapping is.
fn large(layout: Layout) -> bool {
    layout.size() >= LARGE && layout.align() <= MAPPED_ALIGN
}

/// The length mapped for a block of `size` bytes: rounded up to a quarter of its power of two,
/// so a block freed by one page fits the next page of about the same size. A block holds at most
/// a quarter more than it was asked for, and pages it never touches cost no memory.
fn mapped_len(size: usize) -> usize {
    let top = usize::BITS - 1 - size.leading_zeros();
    let step = 1usize << top.saturating_sub(2);
    size.div_ceil(step) * step
}

/// The freed blocks kept for reuse, as address and mapped length, 0 for an empty slot. Guarded
/// by a spin lock rather than a mutex, which could allocate the first time it is taken.
struct Kept {
    busy: AtomicBool,
    slots: UnsafeCell<[(usize, usize); SLOTS]>,
}

// SAFETY: `slots` is only reached through `Kept::with`, which holds `busy` for the access.
unsafe impl Sync for Kept {}

static KEPT: Kept =
    Kept { busy: AtomicBool::new(false), slots: UnsafeCell::new([(0, 0); SLOTS]) };

impl Kept {
    fn with<T>(&self, f: impl FnOnce(&mut [(usize, usize); SLOTS]) -> T) -> T {
        while self
            .busy
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            std::hint::spin_loop();
        }
        // SAFETY: `busy` is held, so no other thread is in here.
        let out = f(unsafe { &mut *self.slots.get() });
        self.busy.store(false, Ordering::Release);
        out
    }

    /// A kept block of exactly `len` bytes, taken out of the slots.
    fn take(&self, len: usize) -> Option<*mut u8> {
        self.with(|slots| {
            let slot = slots.iter_mut().find(|s| s.0 != 0 && s.1 == len)?;
            let addr = slot.0;
            *slot = (0, 0);
            Some(addr as *mut u8)
        })
    }

    /// Keeps a freed block; false when every slot is taken.
    fn keep(&self, addr: *mut u8, len: usize) -> bool {
        self.with(|slots| match slots.iter_mut().find(|s| s.0 == 0) {
            Some(slot) => {
                *slot = (addr as usize, len);
                true
            }
            None => false,
        })
    }

    fn take_all(&self) -> [(usize, usize); SLOTS] {
        self.with(|slots| std::mem::replace(slots, [(0, 0); SLOTS]))
    }
}

const PROT_READ: c_int = 0x01;
const PROT_WRITE: c_int = 0x02;
const MAP_PRIVATE: c_int = 0x0002;
const MAP_ANON: c_int = 0x1000;
const MAP_FAILED: *mut c_void = !0 as *mut c_void;

unsafe extern "C" {
    fn mmap(
        addr: *mut c_void,
        len: usize,
        prot: c_int,
        flags: c_int,
        fd: c_int,
        offset: i64,
    ) -> *mut c_void;
    fn munmap(addr: *mut c_void, len: usize) -> c_int;
}

/// Fresh zeroed pages from the system, or null.
fn map(len: usize) -> *mut u8 {
    // SAFETY: an anonymous private mapping at an address of the system's choosing touches no
    // memory of ours.
    let addr = unsafe {
        mmap(std::ptr::null_mut(), len, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANON, -1, 0)
    };
    if addr == MAP_FAILED { std::ptr::null_mut() } else { addr.cast() }
}

/// Gives a block from [`map`] back to the system.
///
/// # Safety
///
/// `addr` and `len` are a block [`map`] returned, which nothing uses any more.
unsafe fn unmap(addr: *mut u8, len: usize) {
    // SAFETY: the caller hands over a whole mapping nothing uses.
    unsafe { munmap(addr.cast(), len) };
}

/// Gives every kept block back to the system.
pub(crate) fn release() {
    for (addr, len) in KEPT.take_all() {
        if addr != 0 {
            // SAFETY: a kept block is a whole mapping that was freed, so nothing uses it.
            unsafe { unmap(addr as *mut u8, len) };
        }
    }
}

// SAFETY: a large block is a mapping of at least its size, aligned to a page, and is in use by
// one owner at a time: it is in the slots only after it was freed and leaves them before it is
// handed out again. Everything else is the system allocator's, with the layout it was given.
unsafe impl GlobalAlloc for Blocks {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !large(layout) {
            // SAFETY: the caller's layout, passed on as it is.
            return unsafe { System.alloc(layout) };
        }
        let len = mapped_len(layout.size());
        KEPT.take(len).unwrap_or_else(|| map(len))
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if !large(layout) {
            // SAFETY: the caller's layout, passed on as it is.
            return unsafe { System.alloc_zeroed(layout) };
        }
        let len = mapped_len(layout.size());
        match KEPT.take(len) {
            Some(addr) => {
                // SAFETY: a kept block of `len` bytes, now this caller's alone.
                unsafe { addr.write_bytes(0, layout.size()) };
                addr
            }
            None => map(len),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if !large(layout) {
            // SAFETY: a block the system allocator gave out with this layout.
            return unsafe { System.dealloc(ptr, layout) };
        }
        let len = mapped_len(layout.size());
        if !KEPT.keep(ptr, len) {
            // SAFETY: the caller frees the whole block, so nothing uses it.
            unsafe { unmap(ptr, len) };
        }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller guarantees `new_size` is valid for the layout's alignment.
        let new = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        match (large(layout), large(new)) {
            // SAFETY: a block the system allocator gave out with this layout.
            (false, false) => return unsafe { System.realloc(ptr, layout, new_size) },
            (true, true) if mapped_len(layout.size()) == mapped_len(new_size) => return ptr,
            _ => {}
        }
        // SAFETY: `new` is a valid layout, as above.
        let moved = unsafe { self.alloc(new) };
        if !moved.is_null() {
            // SAFETY: both blocks hold at least the smaller size, and are different blocks.
            unsafe { std::ptr::copy_nonoverlapping(ptr, moved, layout.size().min(new_size)) };
            // SAFETY: the old block, with its own layout, which nothing uses now.
            unsafe { self.dealloc(ptr, layout) };
        }
        moved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kept() -> usize {
        KEPT.with(|slots| slots.iter().filter(|s| s.0 != 0).count())
    }

    #[test]
    fn a_length_rounds_up_to_a_quarter_of_its_power_of_two() {
        assert_eq!(mapped_len(LARGE), LARGE);
        assert_eq!(mapped_len(LARGE + 1), LARGE + LARGE / 4);
        assert_eq!(mapped_len(3 * LARGE + 1), 3 * LARGE + LARGE / 2);
        assert_eq!(mapped_len(62_208_000), 64 << 20);
        assert!(mapped_len(62_208_000).is_multiple_of(MAPPED_ALIGN));
    }

    // One test for the kept blocks, as they are the process's own.
    #[test]
    fn a_freed_block_is_kept_for_the_next_one_until_released() {
        let blocks = Blocks;
        let page = Layout::from_size_align(5 * LARGE, 16).unwrap();
        // SAFETY: each block is used within its layout and freed once, with that layout.
        unsafe {
            release();
            let first = blocks.alloc(page);
            assert!(!first.is_null());
            first.write_bytes(7, page.size());
            blocks.dealloc(first, page);
            assert_eq!(kept(), 1);

            // About the same size: the same block, zeroed when asked.
            let near = Layout::from_size_align(5 * LARGE - 4096, 16).unwrap();
            let second = blocks.alloc_zeroed(near);
            assert_eq!(second, first);
            assert_eq!(kept(), 0);
            assert!(std::slice::from_raw_parts(second, near.size()).iter().all(|b| *b == 0));

            // Grown past its length, it moves and keeps what it held.
            second.write_bytes(9, near.size());
            let grown = blocks.realloc(second, near, 9 * LARGE);
            assert!(!grown.is_null());
            assert!(std::slice::from_raw_parts(grown, near.size()).iter().all(|b| *b == 9));
            assert_eq!(kept(), 1);

            // Shrunk below a large block, it goes to the system allocator, contents and all.
            let small =
                blocks.realloc(grown, Layout::from_size_align(9 * LARGE, 16).unwrap(), 64);
            assert!(std::slice::from_raw_parts(small, 64).iter().all(|b| *b == 9));
            blocks.dealloc(small, Layout::from_size_align(64, 16).unwrap());
            assert_eq!(kept(), 2);

            release();
            assert_eq!(kept(), 0);
        }
    }

    #[test]
    fn a_small_or_overaligned_block_is_the_systems() {
        assert!(!large(Layout::from_size_align(LARGE - 1, 8).unwrap()));
        assert!(!large(Layout::from_size_align(LARGE, 2 * MAPPED_ALIGN).unwrap()));
        assert!(large(Layout::from_size_align(LARGE, MAPPED_ALIGN).unwrap()));
    }
}
