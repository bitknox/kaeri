#![no_main]

mod common;

use kaeri_books::Pdf;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(pdf) = Pdf::open(data.to_vec(), &common::limits()) else {
        return;
    };
    let _ = pdf.info();
    for i in 0..pdf.page_count().min(4) {
        let _ = pdf.page_size(i);
        let _ = pdf.page_image(i);
    }
    // Rendering is the slow part; one small render per input keeps the fuzzer moving.
    let _ = pdf.render(0, 96, 96);
});
