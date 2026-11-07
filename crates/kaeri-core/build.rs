//! Checks the reading demo's bundled story cover against the SHA-1 pinned for it, so a build
//! never ships an image other than the one the demo credits. The pin is handed to the crate as
//! `KAERI_DEMO_STORY_COVER_SHA1`, which `reading_demo::STORY_COVER_SHA1` reads.

use sha1::{Digest, Sha1};

/// `assets/demo/kwaidan-yuki-onna-cover.jpg`: the 1904 frontispiece, made 900 pixels tall.
const STORY_COVER_SHA1: &str = "3ca8076ad8eb97419838eb2f71baf7979240da91";

fn main() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/demo/kwaidan-yuki-onna-cover.jpg");
    println!("cargo:rerun-if-changed={}", path.display());
    println!("cargo:rerun-if-changed=build.rs");
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("the reading demo's story cover {}: {e}", path.display()));
    let found: String = Sha1::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        found, STORY_COVER_SHA1,
        "the reading demo's story cover is not the pinned file: see assets/demo/NOTICE.md"
    );
    println!("cargo:rustc-env=KAERI_DEMO_STORY_COVER_SHA1={STORY_COVER_SHA1}");
}
