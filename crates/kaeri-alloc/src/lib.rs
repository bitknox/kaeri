//! The allocator the Apple apps' engine runs on, for the memory the reader works through.
//!
//! The system allocator on macOS and iOS keeps the large blocks a program frees, for the next
//! ones it asks for, and nothing public asks it to give them back. The reader works through
//! pages tens of megabytes each, so after it closed hundreds of megabytes stayed with the app.
//! [`Blocks`] takes blocks of [`LARGE`] bytes and over from the system's pages itself. It keeps
//! the ones freed for the next pages the same way, and [`release`] gives every one it keeps back
//! to the system, which the engine does as the reader closes. Smaller blocks go to the system
//! allocator as they would without it.
//!
//! `kaeri-ffi` installs it as the global allocator on Apple platforms only. Elsewhere [`release`]
//! has nothing to give back.

#[cfg(target_vendor = "apple")]
mod blocks;

#[cfg(target_vendor = "apple")]
pub use blocks::Blocks;

/// The smallest block [`Blocks`] takes from the system's pages itself.
pub const LARGE: usize = 1 << 20;

/// Gives every large block kept for reuse back to the system. Blocks still in use stay theirs
/// and are kept again when they are freed.
pub fn release() {
    #[cfg(target_vendor = "apple")]
    blocks::release();
}
