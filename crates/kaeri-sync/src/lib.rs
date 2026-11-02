//! Cross-device sync, the half that is not tied to the store's tables.
//!
//! One encrypted change log per device travels two roads: through a folder the person already
//! has (this crate's [`folder`] targets), and between paired devices on a network (`peer`). The
//! rules that merge it are the store's (`kaeri_store::sync`); this crate only moves sealed bytes.

pub mod folder;
pub mod google;
pub mod nextcloud;
pub mod webdav;
