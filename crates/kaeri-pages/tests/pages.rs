//! Whole pages through the pipeline, for the panels S1 measured on real readers: a grey
//! 1264x1680 and 1860x2480 at 300 dpi, a Kaleido 3 colour 1264x1680, and an LCD phone. The
//! pages are generated in `kaeri_pages::testpages` (screentone, hatching, bubbles), so no real
//! artwork is in the repository; they are encoded as JPEG, as most comic archives carry them.

use kaeri_pages::layout::{Direction, LayoutOptions, PageShape, lay_out};
use kaeri_pages::resize::Fit;
use kaeri_pages::strip::{StripOptions, extract, gap_rows, plan_cuts};
use kaeri_pages::{
    ColourChoice, Delivery, Look, Panel, PixelFormat, Renderer, SourceFormat, Target, delivery,
    testpages,
};

const GO_COLOR_7: Target = Target { panel: Panel::Kaleido3, width: 1264, height: 1680 };
const GO_10: Target = Target { panel: Panel::Grey, width: 1860, height: 2480 };
const GREY_7: Target = Target { panel: Panel::Grey, width: 1264, height: 1680 };
const PHONE: Target = Target { panel: Panel::Lcd, width: 1179, height: 2556 };

fn grey_jpeg(w: u32, h: u32) -> Vec<u8> {
    testpages::jpeg(&testpages::manga_page(w, h, 7), 90)
}

fn on_sixteen_greys(bytes: &[u8]) -> bool {
    bytes.iter().all(|v| v % 17 == 0)
}

#[test]
fn a_grey_page_on_grey_eink_is_dithered_greys_at_panel_size() {
    for (target, (sw, sh)) in [(GREY_7, (1600, 2133)), (GO_10, (2400, 3200))] {
        let mut r = Renderer::new(target, Look::default());
        let page = r.open(&grey_jpeg(sw, sh)).unwrap();
        assert!(!page.colour && !page.spread);
        assert_eq!(page.format, SourceFormat::Jpeg);
        let out = r.render(&page, None, target.width, target.height).unwrap();
        assert_eq!(out.format(), PixelFormat::Gray8);
        assert_eq!(out.height(), target.height);
        assert!(out.width() <= target.width && out.width() > target.width - 10);
        assert_eq!(out.stride(), out.width());
        assert_eq!(out.bytes().len(), (out.width() * out.height()) as usize);
        assert!(on_sixteen_greys(out.bytes()));
        // Autocontrast reached both ends.
        assert!(out.bytes().contains(&0) && out.bytes().contains(&255));
    }
}

#[test]
fn the_same_page_gives_the_same_bytes() {
    let bytes = grey_jpeg(1600, 2133);
    let mut r = Renderer::new(GREY_7, Look::default());
    let a = r.render(&r.open(&bytes).unwrap(), None, 1264, 1680).unwrap();
    let b = r.render(&r.open(&bytes).unwrap(), None, 1264, 1680).unwrap();
    assert_eq!(a.bytes(), b.bytes());
}

#[test]
fn a_colour_page_on_kaleido_stays_colour_with_opaque_rgba() {
    let bytes = testpages::jpeg(&testpages::colour_page(1600, 2133, 3), 90);
    let mut r = Renderer::new(GO_COLOR_7, Look::default());
    let page = r.open(&bytes).unwrap();
    assert!(page.colour);
    let out = r.render(&page, None, 1264, 1680).unwrap();
    assert!(out.colour);
    assert_eq!(out.format(), PixelFormat::Rgba8);
    assert_eq!(out.stride(), out.width() * 4);
    assert!(out.bytes().as_chunks::<4>().0.iter().all(|p| p[3] == 255));
    // No dither on Kaleido.
    assert!(!on_sixteen_greys(out.bytes()));
}

#[test]
fn a_colour_page_on_grey_eink_goes_grey_and_keeps_its_colour_flag() {
    let bytes = testpages::jpeg(&testpages::colour_page(1600, 2133, 3), 90);
    let mut r = Renderer::new(GREY_7, Look::default());
    let page = r.open(&bytes).unwrap();
    assert_eq!(page.bitmap.format(), PixelFormat::Gray8);
    let out = r.render(&page, None, 1264, 1680).unwrap();
    assert!(out.colour);
    assert_eq!(out.format(), PixelFormat::Gray8);
    assert!(on_sixteen_greys(out.bytes()));
}

#[test]
fn a_yellowed_scan_is_black_and_white_through_jpeg() {
    let scan = testpages::yellowed(&testpages::manga_page(1200, 1600, 4));
    let bytes = testpages::jpeg(&scan, 85);
    let r = Renderer::new(GO_COLOR_7, Look::default());
    let page = r.open(&bytes).unwrap();
    assert!(!page.colour);
    assert_eq!(page.bitmap.format(), PixelFormat::Gray8);

    // The series' choice wins over the check.
    let colour = Look { colour: ColourChoice::Colour, ..Look::default() };
    assert!(Renderer::new(GO_COLOR_7, colour).open(&bytes).unwrap().colour);
    let greys = Look { colour: ColourChoice::Greys, ..Look::default() };
    let colour_page = testpages::jpeg(&testpages::colour_page(800, 1000, 2), 90);
    assert!(!Renderer::new(GO_COLOR_7, greys).open(&colour_page).unwrap().colour);
}

#[test]
fn black_and_white_on_kaleido_takes_the_rainbow_filter_only_when_on() {
    let bytes = grey_jpeg(1600, 2133);
    let off = {
        let mut r = Renderer::new(GO_COLOR_7, Look::default());
        r.render(&r.open(&bytes).unwrap(), None, 1264, 1680).unwrap()
    };
    let on = {
        let look = Look { rainbow_filter: true, ..Look::default() };
        let mut r = Renderer::new(GO_COLOR_7, look).with_threads(2);
        r.render(&r.open(&bytes).unwrap(), None, 1264, 1680).unwrap()
    };
    assert_eq!(off.format(), PixelFormat::Gray8);
    assert_eq!(on.format(), PixelFormat::Gray8);
    assert!(!on_sixteen_greys(off.bytes()), "no dither on Kaleido");
    let changed = off.bytes().iter().zip(on.bytes()).filter(|(a, b)| a != b).count();
    assert!(changed > off.bytes().len() / 20, "the filter changed {changed} pixels");
    // The filter is off by default.
    assert!(!Look::default().rainbow_filter);
}

#[test]
fn crop_trims_blank_margins_up_to_a_tenth_and_never_on_spreads() {
    let page = testpages::with_margin(&testpages::manga_page(1000, 1400, 2), 40, 255);
    let look = Look { crop: true, ..Look::default() };
    let r = Renderer::new(GREY_7, look);
    let opened = r.open(&testpages::jpeg(&page, 90)).unwrap();
    let c = opened.content;
    // The added 40 and the page's own white margin (a thirtieth), within the cap of 108.
    assert!(c.x >= 40 && c.x <= 108, "{c:?}");
    assert!(c.y >= 40 && c.y <= 148, "{c:?}");
    let out = Renderer::new(GREY_7, look).render(&opened, None, 1264, 1680).unwrap();
    assert_eq!(out.height(), 1680);

    let spread = testpages::with_margin(&testpages::manga_page(3000, 2000, 5), 40, 255);
    let opened = r.open(&testpages::jpeg(&spread, 90)).unwrap();
    assert!(opened.spread);
    assert_eq!(opened.content, opened.bitmap.bounds());
}

#[test]
fn a_spread_splits_into_halves_in_reading_order_on_a_single_screen() {
    let cover = grey_jpeg(1600, 2133);
    let spread = testpages::jpeg(&testpages::manga_page(3200, 2133, 8), 90);
    let mut r = Renderer::new(GREY_7, Look::default());
    let pages = [r.open(&cover).unwrap(), r.open(&spread).unwrap()];
    let shapes: Vec<PageShape> =
        pages.iter().map(|p| PageShape::story(p.content.width, p.content.height)).collect();
    let options =
        LayoutOptions { direction: Direction::RightToLeft, ..LayoutOptions::default() };
    let leaves = lay_out(&shapes, options);
    assert_eq!(leaves.len(), 3);
    let first_half = leaves[1].parts[0];
    assert_eq!(first_half.page, 1);
    assert_eq!(first_half.source.unwrap().x, 1600, "right half first");
    for leaf in &leaves {
        let part = leaf.parts[0];
        let out = r.render(&pages[part.page as usize], part.source, 1264, 1680).unwrap();
        assert!(out.width() > 1200 && out.height() == 1680);
    }
}

#[test]
fn lcd_pages_go_as_originals_until_shrunk_past_twice() {
    let small = grey_jpeg(1600, 2133);
    let info = kaeri_pages::inspect(&small).unwrap();
    assert_eq!(delivery(info.format, (info.width, info.height), &PHONE), Delivery::Original);

    let large = grey_jpeg(3000, 4000);
    let info = kaeri_pages::inspect(&large).unwrap();
    assert_eq!(delivery(info.format, (info.width, info.height), &PHONE), Delivery::Processed);
    // Processed for LCD: resized, not toned or dithered.
    let mut r = Renderer::new(PHONE, Look::default());
    let page = r.open(&large).unwrap();
    let out = r.render(&page, None, PHONE.width, PHONE.height).unwrap();
    assert_eq!((out.width(), out.format()), (PHONE.width, PixelFormat::Gray8));
    assert!(!on_sixteen_greys(out.bytes()));

    let colour = testpages::jpeg(&testpages::colour_page(3000, 4000, 1), 90);
    let out = r.render(&r.open(&colour).unwrap(), None, PHONE.width, PHONE.height).unwrap();
    assert_eq!(out.format(), PixelFormat::Rgba8);
}

#[test]
fn a_strip_is_cut_at_its_gaps_on_eink() {
    // Six panels of 900 rows and 80-row gaps, 800 wide, resized to the panel's width.
    let strip = testpages::strip(800, 6, 900, 80);
    let r = Renderer::new(GREY_7, Look::default());
    let page = r.open(&testpages::png(&strip)).unwrap();
    let scaled = r.scale(&page, None, Fit::Width(1264), true).unwrap();
    assert_eq!(scaled.width(), 1264);
    let gaps = gap_rows(&scaled);
    let cuts = plan_cuts(&gaps, StripOptions::for_screen(1680, 47)); // 30 dp at 1.56 px a dp
    assert!(cuts.len() >= 4);
    for cut in &cuts[..cuts.len() - 1] {
        assert!(gaps[cut.bottom as usize - 1], "{cut:?} ends inside a panel");
        assert_eq!(cut.overlap, 0);
        assert!(cut.height() <= 1680);
    }
    assert_eq!(cuts.last().unwrap().bottom, scaled.height());

    let mut r = Renderer::new(GREY_7, Look::default());
    let out = r.finish(extract(&[&scaled], cuts[0]).unwrap(), false).unwrap();
    assert_eq!((out.width(), out.height()), (1264, cuts[0].height()));
    assert!(on_sixteen_greys(out.bytes()));
}

#[test]
fn a_4_bit_png_export_of_a_page() {
    let r = Renderer::new(GREY_7, Look::default());
    let page = r.open(&grey_jpeg(1600, 2133)).unwrap();
    let mut scaled =
        r.scale(&page, None, Fit::Within { width: 1264, height: 1680 }, false).unwrap();
    kaeri_pages::dither::floyd_steinberg(&mut scaled).unwrap();
    let png = kaeri_pages::dither::png_grey4(&scaled).unwrap();
    assert!(png.len() < scaled.data().len() / 2);
    let info = kaeri_pages::inspect(&png).unwrap();
    assert_eq!(
        (info.format, info.width, info.height),
        (SourceFormat::Png, scaled.width(), 1680)
    );
}
