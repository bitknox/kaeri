#![no_main]

mod common;

use kaeri_books::archive::ArchiveKind;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| common::archive(data, ArchiveKind::SevenZip));
