//! Cross-device sync, the half that is not tied to the store's tables.
//!
//! One encrypted change log per device travels two roads: through a folder the person already
//! has (this crate's [`folder`] targets and [`road`]), and between paired devices on a network
//! ([`peer`]). The rules that merge it are the store's (`kaeri_store::sync`); this crate only
//! moves sealed bytes.

pub mod dropbox;
pub mod error;
#[cfg(test)]
mod fakehttp;
pub mod folder;
pub mod google;
pub mod keys;
pub mod lan;
pub mod loopback;
pub mod nextcloud;
pub mod onedrive;
pub mod peer;
pub(crate) mod rest;
pub mod road;
pub mod seal;
pub mod webdav;

pub use error::{Result, SyncError};
