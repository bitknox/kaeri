//! Round 5 engine audit: generated inputs for the page pipeline. Every entry point that takes a
//! page from a book or a source (inspect, decode, fingerprint, render, layout, strip cuts) is fed
//! odd sizes and damaged files, and must answer with a value or an error, never a panic, a
//! non-finite place or a hang.

use std::panic::{AssertUnwindSafe, catch_unwind};

use kaeri_pages::{
    Bitmap, ColourChoice, DecodeLimits, Look, Panel, PixelFormat, Renderer, SourceFormat,
    Target, decode, decode_for, inspect,
    layout::{Direction, LayoutOptions, PageKind, PageShape, Screen, SpreadOnSingle, lay_out},
    print::fingerprint,
    strip::{StripOptions, extract, plan_cuts},
    testpages,
};

/// xorshift64*: enough randomness for generated inputs, and the same run every time.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u64) as usize]
    }
}

const ODD_SIDES: [u32; 10] = [0, 1, 2, 3, 7, 64, 999, 1000, 1401, 4000];

fn options(rng: &mut Rng) -> LayoutOptions {
    LayoutOptions {
        screen: rng.pick(&[Screen::Single, Screen::Double]),
        direction: rng.pick(&[Direction::LeftToRight, Direction::RightToLeft]),
        spread_on_single: rng.pick(&[
            SpreadOnSingle::Split,
            SpreadOnSingle::Rotate,
            SpreadOnSingle::Whole,
        ]),
        first_is_cover: rng.below(2) == 0,
    }
}

/// R5E-1. A page whose header the engine cannot read is laid out as 0x0 (pages.rs
/// `keep_original`, `original_at`). Two such pages side by side on a two-page screen get a NaN
/// place, which Android's `LeafView` rounds with `roundToInt()` and throws.
#[test]
#[ignore = "R5E-1: two 0x0 pages pair with a NaN place"]
fn two_unreadable_pages_on_a_two_page_screen_get_finite_places() {
    let unread = PageShape::story(0, 0);
    let options =
        LayoutOptions { screen: Screen::Double, first_is_cover: false, ..Default::default() };
    let leaves = lay_out(&[unread, unread], options);
    for part in leaves.iter().flat_map(|l| &l.parts) {
        let p = part.place;
        assert!(
            [p.x, p.y, p.width, p.height].iter().all(|v| v.is_finite()),
            "page {} placed at {p:?}",
            part.page
        );
    }
}

/// Every leaf part sits inside the leaf, every page that is not skipped is shown, and pages
/// come in order, for any sizes a book can give (zero included).
#[test]
fn layout_of_any_sizes_keeps_every_page_in_order() {
    let mut rng = Rng(0x5eed_0001);
    for round in 0..20_000 {
        let n = rng.below(9) as usize;
        let pages: Vec<PageShape> = (0..n)
            .map(|_| PageShape {
                // Zero sides are R5E-1's; this test covers every other size.
                width: rng.pick(&ODD_SIDES[1..]),
                height: rng.pick(&ODD_SIDES[1..]),
                kind: rng.pick(&[
                    PageKind::Story,
                    PageKind::Story,
                    PageKind::Cover,
                    PageKind::Skip,
                ]),
            })
            .collect();
        let opts = options(&mut rng);
        let leaves = lay_out(&pages, opts);
        let mut seen = Vec::new();
        for leaf in &leaves {
            assert!(!leaf.parts.is_empty(), "round {round}: empty leaf");
            for part in &leaf.parts {
                let p = part.place;
                let inside = [p.x, p.y, p.width, p.height].iter().all(|v| v.is_finite())
                    && p.x >= 0.0
                    && p.y >= 0.0
                    && p.x + p.width <= 1.0 + 1e-5
                    && p.y + p.height <= 1.0 + 1e-5;
                assert!(inside, "round {round}: {pages:?} {opts:?} gave {p:?}");
                if let Some(s) = part.source {
                    let page = pages[part.page as usize];
                    assert!(
                        s.width > 0 && s.x + s.width <= page.width && s.height <= page.height,
                        "round {round}: half {s:?} outside {page:?}"
                    );
                }
                if seen.last() != Some(&part.page) {
                    seen.push(part.page);
                }
            }
        }
        let shown: Vec<u32> =
            (0..n as u32).filter(|&i| pages[i as usize].kind != PageKind::Skip).collect();
        assert_eq!(seen, shown, "round {round}: {pages:?} {opts:?}");
    }
}

/// Strip cuts cover every row once (plus overlaps), never exceed a screen and always end.
#[test]
fn strip_cuts_cover_every_row_for_any_gaps() {
    let mut rng = Rng(0x5eed_0002);
    for round in 0..5_000 {
        let len = rng.below(3000) as usize;
        let density = rng.below(4);
        let gaps: Vec<bool> = (0..len).map(|_| rng.below(4) < density).collect();
        let screen = rng.pick(&[0, 1, 2, 3, 5, 50, 800, 100_000]);
        let opts = StripOptions {
            screen_height: screen,
            overlap: rng.pick(&[0, 1, 30, 400, u32::MAX]),
            search: rng.pick(&[0, 1, 200, u32::MAX]),
        };
        let cuts = plan_cuts(&gaps, opts);
        let mut covered = 0u32;
        for c in &cuts {
            assert!(c.top <= covered, "round {round}: gap before {c:?}");
            assert!(c.bottom > c.top, "round {round}: empty cut {c:?}");
            assert!(c.height() <= screen.max(1), "round {round}: {c:?} over {screen}");
            covered = c.bottom;
        }
        assert_eq!(covered as usize, len, "round {round}: {opts:?}");
    }
}

#[test]
fn extract_of_any_cut_errs_or_fits() {
    let mut rng = Rng(0x5eed_0003);
    for _ in 0..2_000 {
        let w = rng.pick(&[1, 2, 5, 33]);
        let images: Vec<Bitmap> = (0..rng.below(4))
            .map(|_| Bitmap::filled(w, rng.below(40) as u32, PixelFormat::Gray8, 200))
            .collect();
        let refs: Vec<&Bitmap> = images.iter().collect();
        let total: u32 = images.iter().map(Bitmap::height).sum();
        let top = rng.below(total as u64 + 3) as u32;
        let bottom = top + rng.below(30) as u32;
        let cut = kaeri_pages::strip::StripCut { top, bottom, overlap: rng.below(50) as u32 };
        let _ = catch_unwind(AssertUnwindSafe(|| extract(&refs, cut)))
            .unwrap_or_else(|_| panic!("extract panicked on {cut:?} over {total} rows"));
    }
}

fn seeds() -> Vec<Vec<u8>> {
    let page = testpages::manga_page(48, 64, 7);
    let colour = testpages::colour_page(300, 420, 3);
    let mut out =
        vec![testpages::jpeg(&page, 80), testpages::png(&page), testpages::png(&colour)];
    let rgb = image::RgbImage::from_raw(300, 420, colour.data().to_vec()).unwrap();
    for format in [image::ImageFormat::Gif, image::ImageFormat::WebP, image::ImageFormat::Jpeg]
    {
        let mut bytes = Vec::new();
        let img = image::DynamicImage::ImageRgb8(rgb.clone());
        let img = if format == image::ImageFormat::WebP { img.to_rgba8().into() } else { img };
        img.write_to(&mut std::io::Cursor::new(&mut bytes), format).unwrap();
        out.push(bytes);
    }
    out
}

fn mutate(rng: &mut Rng, seed: &[u8]) -> Vec<u8> {
    let mut b = seed.to_vec();
    match rng.below(5) {
        0 => b.truncate(rng.below(b.len() as u64 + 1) as usize),
        1 => {
            for _ in 0..=rng.below(8) {
                let i = rng.below(b.len() as u64) as usize;
                if let Some(v) = b.get_mut(i) {
                    *v = rng.next() as u8;
                }
            }
        }
        2 => {
            // Size fields live in the first bytes of every format: hit those.
            for _ in 0..=rng.below(4) {
                let i = rng.below(b.len().min(200) as u64) as usize;
                if let Some(v) = b.get_mut(i) {
                    *v = rng.pick(&[0, 0xFF, 0x7F, 0x80, 1]);
                }
            }
        }
        3 => {
            let i = rng.below(b.len() as u64) as usize;
            let extra: Vec<u8> = (0..rng.below(64)).map(|_| rng.next() as u8).collect();
            b.splice(i..i, extra);
        }
        _ => {
            let i = rng.below(b.len() as u64) as usize;
            let j = (i + rng.below(64) as usize).min(b.len());
            b.drain(i..j);
        }
    }
    b
}

/// Damaged page files never panic any reader of page bytes, and a file whose header gives a
/// size decodes to that size or not at all.
#[test]
fn damaged_page_files_err_without_panicking() {
    let seeds = seeds();
    let mut rng = Rng(0x5eed_0004);
    let small = DecodeLimits { max_pixels: 100_000 };
    for round in 0..6_000 {
        let seed = &seeds[round % seeds.len()];
        let bytes = mutate(&mut rng, seed);
        let room = rng.pick(&[(1, 1), (0, 0), (36, 32), (1860, 2480)]);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let info = inspect(&bytes).ok();
            let a = decode(&bytes, small).ok();
            let b = decode_for(&bytes, small, Some(room)).ok();
            let _ = fingerprint(&bytes);
            (info, a, b)
        }));
        let Ok((info, a, _)) = result else {
            let dir = std::env::temp_dir().join("kaeri-r5-pages");
            let _ = std::fs::create_dir_all(&dir);
            let path = dir.join(format!("round-{round}.bin"));
            let _ = std::fs::write(&path, &bytes);
            panic!("round {round}: a reader panicked; input in {}", path.display());
        };
        if let (Some(info), Some(a)) = (info, a) {
            assert_eq!(
                (a.width(), a.height()),
                (info.width, info.height),
                "round {round}: decoded size differs from the header"
            );
        }
    }
}

/// Any decoded size goes through every panel's pipeline into any screen without a panic.
#[test]
fn every_panel_renders_odd_sizes() {
    let sizes = [(1, 1), (1, 3000), (3000, 1), (2, 2), (5, 3), (3, 5), (1201, 1000)];
    // Send to e-reader keeps a screen between 100 and 8000 pixels a side (export/profiles.rs);
    // the reader's own targets are its view's size, never 0.
    let targets = [(1, 1), (2, 3), (1, 2000), (100, 100), (100, 8000), (1860, 2480)];
    for &(w, h) in &sizes {
        for format in [PixelFormat::Gray8, PixelFormat::Rgb8] {
            // The drawn test pages need room for their panels; small sizes are a flat fill.
            let drawn = w >= 300 && h >= 300;
            let page = match format {
                PixelFormat::Gray8 if drawn => testpages::manga_page(w, h, 1),
                _ if drawn => testpages::colour_page(w, h, 1),
                _ => Bitmap::filled(w, h, format, 180),
            };
            let bytes = testpages::png(&page);
            for panel in [
                Panel::Lcd,
                Panel::Grey,
                Panel::Kaleido3,
                Panel::Gallery3,
                Panel::UnknownColour,
            ] {
                for &(tw, th) in &targets {
                    for crop in [false, true] {
                        let look = Look {
                            crop,
                            rainbow_filter: true,
                            colour: ColourChoice::Auto,
                            ..Look::default()
                        };
                        let target = Target { panel, width: tw, height: th };
                        let run = catch_unwind(AssertUnwindSafe(|| {
                            let mut r = Renderer::new(target, look);
                            let Ok(src) = r.open(&bytes) else { return };
                            let _ = r.render(&src, None, tw, th);
                            let _ = kaeri_pages::export::prepare(
                                &mut r,
                                &bytes,
                                kaeri_pages::layout::Direction::RightToLeft,
                            );
                        }));
                        assert!(
                            run.is_ok(),
                            "{w}x{h} {format:?} on {panel:?} {tw}x{th} crop {crop} panicked"
                        );
                    }
                }
            }
        }
    }
    let _ = SourceFormat::sniff(&[]);
}
