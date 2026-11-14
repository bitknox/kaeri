//! Generated test pages, for this crate's tests and for any other crate's (the testkit and
//! the Fixtures scenes). Every page is drawn here in code, so it carries the repository's
//! licence and shows no real artwork: panel borders, dot screentones at several periods and
//! angles, line hatching, dot gradients and speech bubbles with text-like strokes, a zone
//! plate for resampling, a strip with gaps between panels.
//!
//! Only built with the `testing` feature.

use std::io::Cursor;

use crate::{Bitmap, PixelFormat};

/// A small linear congruential generator, so pages are the same on every run.
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg(seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407))
    }

    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) as u32
    }

    fn range(&mut self, lo: u32, hi: u32) -> u32 {
        lo + self.next() % (hi - lo).max(1)
    }
}

/// A zone plate: rings whose frequency grows from 0 at the centre to `max_frequency` cycles
/// a pixel at the middle of the nearest edge. Every frequency and direction up to that is
/// somewhere on it, so aliasing shows as false rings.
pub fn zone_plate(width: u32, height: u32, max_frequency: f32) -> Bitmap {
    let (cx, cy) = (width as f32 / 2.0, height as f32 / 2.0);
    let r_max = cx.min(cy);
    let data = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .map(|(x, y)| {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            let phase = std::f32::consts::PI * max_frequency * (dx * dx + dy * dy) / r_max;
            (127.5 + 127.5 * phase.cos()).round() as u8
        })
        .collect();
    Bitmap::new(width, height, PixelFormat::Gray8, data).expect("sized to fit")
}

/// The zone plate's local frequency, in cycles a pixel, at a point.
pub fn zone_plate_frequency(
    width: u32,
    height: u32,
    max_frequency: f32,
    x: f32,
    y: f32,
) -> f32 {
    let (cx, cy) = (width as f32 / 2.0, height as f32 / 2.0);
    let r = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
    max_frequency * r / cx.min(cy)
}

/// A black and white manga-like page with white margins of a thirtieth of its short side.
pub fn manga_page(width: u32, height: u32, seed: u64) -> Bitmap {
    let (w, h) = (width as usize, height as usize);
    let mut px = vec![255u8; w * h];
    let mut rng = Lcg::new(seed);
    let m = (w.min(h) / 30).max(8);
    let rows = 2 + (rng.next() % 2) as usize;
    let mut y0 = m;
    for r in 0..rows {
        let y1 = if r + 1 == rows { h - m } else { y0 + (h - 2 * m) / rows - m / 2 };
        let cols = 1 + (rng.next() % 2) as usize;
        let mut x0 = m;
        for c in 0..cols {
            let x1 = if c + 1 == cols { w - m } else { x0 + (w - 2 * m) / cols - m / 2 };
            let panel = Panel {
                x0,
                y0,
                x1,
                y1,
                kind: rng.next() % 4,
                period: rng.range(4, 9) as f32,
                angle: if rng.next().is_multiple_of(2) { 45f32 } else { 30f32 }.to_radians(),
            };
            panel.fill(&mut px, w, &mut rng);
            frame(&mut px, w, &panel, (m / 4).max(3));
            x0 = x1 + m / 2;
        }
        y0 = y1 + m / 2;
    }
    Bitmap::new(width, height, PixelFormat::Gray8, px).expect("sized to fit")
}

struct Panel {
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
    kind: u32,
    period: f32,
    angle: f32,
}

impl Panel {
    fn fill(&self, px: &mut [u8], w: usize, rng: &mut Lcg) {
        let (s, c) = self.angle.sin_cos();
        let (pw, ph) = ((self.x1 - self.x0) as f32, (self.y1 - self.y0) as f32);
        for y in self.y0..self.y1 {
            for x in self.x0..self.x1 {
                let (fx, fy) = (x as f32, y as f32);
                let u = (fx * c + fy * s) / self.period;
                let v = (-fx * s + fy * c) / self.period;
                let (du, dv) = (u - u.round(), v - v.round());
                let d2 = du * du + dv * dv;
                px[y * w + x] = match self.kind {
                    // A flat dot screen.
                    0 => {
                        if d2 < 0.12 {
                            0
                        } else {
                            255
                        }
                    }
                    // A gradient: the dots grow across the panel.
                    1 => {
                        let t = (x - self.x0) as f32 / pw;
                        if d2 < 0.02 + 0.22 * t { 0 } else { 255 }
                    }
                    // Line hatching.
                    2 => {
                        if du.abs() < 0.18 {
                            30
                        } else {
                            250
                        }
                    }
                    // A soft sky with a dot screen over its lower half.
                    _ => {
                        let t = (y - self.y0) as f32 / ph;
                        if t > 0.5 && d2 < 0.08 { 20 } else { (255.0 - 90.0 * t) as u8 }
                    }
                };
            }
        }
        // A speech bubble with text-like strokes.
        let (bw, bh) = (((self.x1 - self.x0) / 3).max(20), ((self.y1 - self.y0) / 4).max(20));
        let bx = self.x0 + rng.range(0, (self.x1 - self.x0).saturating_sub(bw) as u32) as usize;
        let by = self.y0 + rng.range(0, (self.y1 - self.y0).saturating_sub(bh) as u32) as usize;
        let (cx, cy) = (bx as f32 + bw as f32 / 2.0, by as f32 + bh as f32 / 2.0);
        for y in by..by + bh {
            for x in bx..bx + bw {
                let ex = (x as f32 - cx) / (bw as f32 / 2.0);
                let ey = (y as f32 - cy) / (bh as f32 / 2.0);
                let r = ex * ex + ey * ey;
                if r < 1.0 {
                    px[y * w + x] = if r > 0.9 { 0 } else { 255 };
                }
            }
        }
        let line_h = (bh / 6).max(4);
        let mut ty = by + bh / 4;
        while ty + line_h < by + 3 * bh / 4 {
            let mut tx = bx + bw / 4;
            while tx + 6 < bx + 3 * bw / 4 {
                let gw = rng.range(3, 7) as usize;
                for y in ty..ty + line_h * 2 / 3 {
                    for x in tx..tx + gw {
                        if !rng.next().is_multiple_of(3) {
                            px[y * w + x] = 0;
                        }
                    }
                }
                tx += gw + 2;
            }
            ty += line_h;
        }
    }
}

fn frame(px: &mut [u8], w: usize, p: &Panel, t: usize) {
    for y in p.y0..p.y1 {
        for x in p.x0..p.x1 {
            if y < p.y0 + t || y >= p.y1 - t || x < p.x0 + t || x >= p.x1 - t {
                px[y * w + x] = 0;
            }
        }
    }
}

/// A colour page: the manga page tinted warm, cool or left neutral by region.
pub fn colour_page(width: u32, height: u32, seed: u64) -> Bitmap {
    let grey = manga_page(width, height, seed);
    let (w, h) = (width as usize, height as usize);
    let data = grey
        .data()
        .iter()
        .enumerate()
        .flat_map(|(i, &v)| {
            let (x, y) = (i % w, i / w);
            let v = v as u16;
            let [r, g, b] = match (x * 3 / w + y * 2 / h) % 3 {
                0 => [v, v * 3 / 4, v / 2],
                1 => [v / 2, v * 3 / 4, v],
                _ => [v, v, v],
            };
            [r as u8, g as u8, b as u8]
        })
        .collect();
    Bitmap::new(width, height, PixelFormat::Rgb8, data).expect("sized to fit")
}

/// A black and white page as an old scan shows it: on yellowed paper, as RGB.
pub fn yellowed(page: &Bitmap) -> Bitmap {
    let paper = [242f32, 230.0, 198.0];
    let data = page
        .clone()
        .into_gray()
        .data()
        .iter()
        .flat_map(|&v| paper.map(|c| (c * v as f32 / 255.0).round() as u8))
        .collect();
    Bitmap::new(page.width(), page.height(), PixelFormat::Rgb8, data).expect("sized to fit")
}

/// A vertical strip `width` wide: framed panels of `panel_height` rows of dot screentone,
/// separated by white gaps of `gap` rows. No row inside a panel is flat.
pub fn strip(width: u32, panels: u32, panel_height: u32, gap: u32) -> Bitmap {
    let frame = 4;
    let mut data = Vec::new();
    for p in 0..panels {
        let period = 5.0 + p as f32 % 4.0;
        for y in 0..panel_height {
            data.extend((0..width).map(|x| {
                let edge =
                    x < frame || x >= width - frame || y < frame || y >= panel_height - frame;
                let (u, v) =
                    ((x + y) as f32 / period, (x + 2 * panel_height - y) as f32 / period);
                let (du, dv) = (u - u.round(), v - v.round());
                if edge || du * du + dv * dv < 0.1 { 0u8 } else { 255 }
            }));
        }
        if p + 1 < panels {
            data.extend(std::iter::repeat_n(255u8, (gap * width) as usize));
        }
    }
    let height = (data.len() / width as usize) as u32;
    Bitmap::new(width, height, PixelFormat::Gray8, data).expect("sized to fit")
}

/// Encodes a grey or RGB bitmap as a JPEG at `quality`.
pub fn jpeg(page: &Bitmap, quality: u8) -> Vec<u8> {
    let mut out = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
    let colour = match page.format() {
        PixelFormat::Gray8 => image::ExtendedColorType::L8,
        PixelFormat::Rgb8 => image::ExtendedColorType::Rgb8,
        PixelFormat::Rgba8 => image::ExtendedColorType::Rgba8,
    };
    image::ImageEncoder::write_image(encoder, page.data(), page.width(), page.height(), colour)
        .expect("in-memory encode");
    out
}

/// Encodes a bitmap as a PNG.
pub fn png(page: &Bitmap) -> Vec<u8> {
    let mut out = Vec::new();
    let colour = match page.format() {
        PixelFormat::Gray8 => image::ExtendedColorType::L8,
        PixelFormat::Rgb8 => image::ExtendedColorType::Rgb8,
        PixelFormat::Rgba8 => image::ExtendedColorType::Rgba8,
    };
    image::write_buffer_with_format(
        &mut Cursor::new(&mut out),
        page.data(),
        page.width(),
        page.height(),
        colour,
        image::ImageFormat::Png,
    )
    .expect("in-memory encode");
    out
}

/// Pads a page with `margin` rows and columns of `paper` on every side.
pub fn with_margin(page: &Bitmap, margin: u32, paper: u8) -> Bitmap {
    let bpp = page.format().bytes_per_pixel();
    let (w, h) = (page.width() + 2 * margin, page.height() + 2 * margin);
    let mut out = Bitmap::filled(w, h, page.format(), paper);
    let stride = w as usize * bpp;
    for y in 0..page.height() {
        let start = (y + margin) as usize * stride + margin as usize * bpp;
        out.data_mut()[start..start + page.stride()].copy_from_slice(page.row(y));
    }
    out
}
