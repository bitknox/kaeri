//! Times the pipeline on generated pages for the two Boox panel sizes S1 measured: 1264x1680
//! (Go Color 7) and 1860x2480 (Go 10.3, Note Air 4C). Local only: no files, no network.
//!
//!     cargo run --release -p kaeri-pages --features testing --example timings

use std::time::Instant;

use kaeri_pages::rainbow::RainbowFilter;
use kaeri_pages::{Look, Panel, Renderer, Target, testpages};

fn median_ms(reps: usize, mut f: impl FnMut()) -> f64 {
    f();
    let mut v: Vec<f64> = (0..reps)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn main() {
    let reps = 9;
    for ((pw, ph), (sw, sh)) in [((1264, 1680), (1600, 2133)), ((1860, 2480), (2400, 3200))] {
        println!("## {pw}x{ph} panel, {sw}x{sh} source");
        let grey = testpages::jpeg(&testpages::manga_page(sw, sh, 7), 90);
        let colour = testpages::jpeg(&testpages::colour_page(sw, sh, 3), 90);
        let large = testpages::jpeg(&testpages::manga_page(sw * 2, sh * 2, 9), 90);

        let target = |panel| Target { panel, width: pw, height: ph };
        let mut grey_panel = Renderer::new(target(Panel::Grey), Look::default());
        let t = median_ms(reps, || {
            let page = grey_panel.open(&grey).unwrap();
            grey_panel.render(&page, None, pw, ph).unwrap();
        });
        println!("grey e-ink, whole chain (decode to dithered page): {t:.1} ms");
        let t = median_ms(reps, || {
            let page = grey_panel.open(&large).unwrap();
            grey_panel.render(&page, None, pw, ph).unwrap();
        });
        println!("grey e-ink, whole chain from a {}x{} source: {t:.1} ms", sw * 2, sh * 2);

        let page = grey_panel.open(&grey).unwrap();
        let t = median_ms(reps, || {
            grey_panel.open(&grey).unwrap();
        });
        println!("  of which decode and checks: {t:.1} ms");
        let t = median_ms(reps, || {
            grey_panel.render(&page, None, pw, ph).unwrap();
        });
        println!("  of which tone, resize and dither: {t:.1} ms");

        let mut kaleido = Renderer::new(target(Panel::Kaleido3), Look::default());
        let t = median_ms(reps, || {
            let page = kaleido.open(&colour).unwrap();
            kaleido.render(&page, None, pw, ph).unwrap();
        });
        println!("Kaleido 3, colour page, whole chain: {t:.1} ms");

        for threads in [1, 2] {
            let look = Look { rainbow_filter: true, ..Look::default() };
            let mut kaleido =
                Renderer::new(target(Panel::Kaleido3), look).with_threads(threads);
            let page = kaleido.open(&grey).unwrap();
            kaleido.render(&page, None, pw, ph).unwrap();
            let t = median_ms(reps.min(5), || {
                kaleido.render(&page, None, pw, ph).unwrap();
            });
            println!(
                "Kaleido 3, black and white page with the rainbow filter, {threads} thread(s), after decode: {t:.1} ms"
            );
        }
        let t0 = Instant::now();
        let filter = RainbowFilter::new(pw, ph);
        let plan = t0.elapsed().as_secs_f64() * 1000.0;
        let mut panel_page = testpages::manga_page(pw, ph, 5);
        let t = median_ms(reps.min(5), || filter.apply(&mut panel_page, 1).unwrap());
        println!(
            "rainbow filter alone at {pw}x{ph} (FFT {:?}): {t:.1} ms, plan and mask {plan:.1} ms, {:.0} MB working",
            filter.fft_size(),
            filter.working_bytes() as f64 / 1e6
        );
        println!();
    }
}
