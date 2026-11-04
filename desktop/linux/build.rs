//! A build from the repository runs on the patched libmpv that `scripts/libmpv.sh` builds into
//! `.libmpv/current`, when it is there for this architecture: the binary gets a DT_RPATH to its
//! `lib`, which the loader searches before the system's folders, for libmpv and for the FFmpeg
//! libraries beside it. So a dev build finds skip times from the audio as a release does, with
//! the demuxer audio tap. Without that build, or with `KAERI_SYSTEM_LIBMPV` set, the app runs on
//! the system's libmpv, which kaeri-mpv-host links through pkg-config either way, and the
//! engine says in Settings that finding is not available.
//!
//! The AppImage sets its own `$ORIGIN` path over this one (`scripts/appimage.sh`). libmpv.sh
//! touches this file when it writes a build, so the next cargo build picks it up.

use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=KAERI_SYSTEM_LIBMPV");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux")
        || env::var_os("KAERI_SYSTEM_LIBMPV").is_some()
    {
        return;
    }
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let current = manifest.join(".libmpv/current");
    let stamp = current.join("stamp");
    let Ok(stamp_text) = std::fs::read_to_string(&stamp) else { return };
    println!("cargo:rerun-if-changed={}", stamp.display());
    // The build is for the machine libmpv.sh ran on, which a cross build is not.
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if !stamp_text.lines().any(|line| line == format!("arch {arch}")) {
        return;
    }
    let lib = current.join("lib");
    if !lib.join("libmpv.so.2").is_file() {
        return;
    }
    // DT_RPATH, not RUNPATH: only DT_RPATH is searched for libmpv's own FFmpeg libraries too.
    println!("cargo:rustc-link-arg-bins=-Wl,--disable-new-dtags,-rpath,{}", lib.display());
    println!("cargo:rustc-link-arg-bins=-Wl,-rpath-link,{}", lib.display());
}
