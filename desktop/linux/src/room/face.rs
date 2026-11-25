//! A person's face: the initial on a disc, and around it one of three ring shapes. Solid is
//! with the room (water, or haze while the room is paused); an arc filling toward the 20 s cap
//! means the room waits for them; no ring and a dimmed face means they are not with the room.
//! The pause badge marks who paused the room, and the vermilion dot an error on their side.

use std::{cell::RefCell, f64::consts::TAU, rc::Rc};

use adw::prelude::*;
use gtk::cairo;

use super::words::{Face as Look, Ring};

const WATER: (f64, f64, f64) = (0x7F as f64 / 255.0, 0xC7 as f64 / 255.0, 0xD9 as f64 / 255.0);
const HAZE: (f64, f64, f64) = (0xA3 as f64 / 255.0, 0xA9 as f64 / 255.0, 0xC4 as f64 / 255.0);
const SILK: (f64, f64, f64) = (0xE4 as f64 / 255.0, 0xE0 as f64 / 255.0, 0xD6 as f64 / 255.0);
const RULE: (f64, f64, f64) = (0x43 as f64 / 255.0, 0x46 as f64 / 255.0, 0x53 as f64 / 255.0);
const DISC: (f64, f64, f64) = (0x34 as f64 / 255.0, 0x3A as f64 / 255.0, 0x57 as f64 / 255.0);
const GROUND: (f64, f64, f64) = (0x16 as f64 / 255.0, 0x1A as f64 / 255.0, 0x2E as f64 / 255.0);
const VERMILION: (f64, f64, f64) =
    (0xE0 as f64 / 255.0, 0x6B as f64 / 255.0, 0x54 as f64 / 255.0);

/// The ring's stroke and the gap inside it, by disc size, as the board draws them.
fn stroke(disc: f64) -> (f64, f64) {
    let width = if disc >= 48.0 {
        3.0
    } else if disc >= 30.0 {
        2.0
    } else {
        1.75
    };
    (width, if disc >= 48.0 { 3.0 } else { 2.0 })
}

/// The widget's side for a disc of `disc` px.
pub fn side(disc: i32) -> i32 {
    let (w, g) = stroke(f64::from(disc));
    disc + (2.0 * (w + g)).round() as i32
}

#[derive(Clone)]
pub struct FaceView {
    pub widget: gtk::DrawingArea,
    state: Rc<RefCell<(String, Look)>>,
}

impl FaceView {
    pub fn new(disc: i32) -> Self {
        let widget = gtk::DrawingArea::new();
        let s = side(disc);
        widget.set_content_width(s);
        widget.set_content_height(s);
        widget.set_valign(gtk::Align::Center);
        widget.set_halign(gtk::Align::Center);
        widget.set_can_target(false);
        let state = Rc::new(RefCell::new((
            String::new(),
            Look {
                word: String::new(),
                ring: Ring::Water,
                dim: 1.0,
                badge: false,
                error: false,
                loud: false,
            },
        )));
        widget.set_draw_func({
            let state = Rc::clone(&state);
            move |area, cr, _, _| {
                let (name, look) = &*state.borrow();
                draw(area, cr, f64::from(disc), name, look);
            }
        });
        Self { widget, state }
    }

    pub fn show(&self, name: &str, look: &Look) {
        let mut state = self.state.borrow_mut();
        if state.0 != name || state.1 != *look {
            *state = (name.to_owned(), look.clone());
            drop(state);
            self.widget.queue_draw();
        }
    }
}

fn rgb(cr: &cairo::Context, (r, g, b): (f64, f64, f64), a: f64) {
    cr.set_source_rgba(r, g, b, a);
}

fn draw(area: &gtk::DrawingArea, cr: &cairo::Context, disc: f64, name: &str, look: &Look) {
    let (width, gap) = stroke(disc);
    let side = disc + 2.0 * (width + gap);
    let c = side / 2.0;
    let r = c - width / 2.0;
    cr.set_line_width(width);
    match look.ring {
        Ring::Water | Ring::Haze => {
            rgb(cr, if look.ring == Ring::Water { WATER } else { HAZE }, 1.0);
            cr.new_sub_path();
            cr.arc(c, c, r, 0.0, TAU);
            let _ = cr.stroke();
        }
        Ring::Progress(frac) => {
            rgb(cr, RULE, 1.0);
            cr.new_sub_path();
            cr.arc(c, c, r, 0.0, TAU);
            let _ = cr.stroke();
            rgb(cr, SILK, 1.0);
            cr.set_line_cap(cairo::LineCap::Round);
            let start = -TAU / 4.0;
            cr.new_sub_path();
            cr.arc(c, c, r, start, start + TAU * frac.clamp(0.04, 1.0));
            let _ = cr.stroke();
        }
        Ring::None => {}
    }
    // The disc and its initial, dimmed with the face.
    let inset = width + gap;
    rgb(cr, DISC, look.dim);
    cr.new_sub_path();
    cr.arc(c, c, disc / 2.0, 0.0, TAU);
    let _ = cr.fill();
    let initial: String =
        name.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
    let layout = area.create_pango_layout(Some(&initial));
    let mut font = pango::FontDescription::new();
    font.set_family("Inter");
    font.set_weight(pango::Weight::Semibold);
    font.set_absolute_size((disc * 0.42).round() * f64::from(pango::SCALE));
    layout.set_font_description(Some(&font));
    let (_, logical) = layout.pixel_extents();
    rgb(cr, SILK, look.dim);
    cr.move_to(
        inset + (disc - f64::from(logical.width())) / 2.0,
        inset + (disc - f64::from(logical.height())) / 2.0,
    );
    pangocairo::functions::show_layout(cr, &layout);
    if look.badge {
        // The pause badge at the bottom trailing edge, on the ground.
        let bz = (disc * 0.42).round().max(12.0);
        let (bx, by) = (side - bz / 2.0 + 1.0, side - bz / 2.0 + 1.0);
        rgb(cr, GROUND, 1.0);
        cr.new_sub_path();
        cr.arc(bx - 2.0, by - 2.0, bz / 2.0, 0.0, TAU);
        let _ = cr.fill();
        rgb(cr, SILK, 1.0);
        let bar = (bz - 4.0) / 24.0;
        let ox = bx - 2.0 - (bz - 4.0) / 2.0;
        let oy = by - 2.0 - (bz - 4.0) / 2.0;
        cr.rectangle(ox + 6.0 * bar, oy + 4.5 * bar, 4.0 * bar, 15.0 * bar);
        cr.rectangle(ox + 14.0 * bar, oy + 4.5 * bar, 4.0 * bar, 15.0 * bar);
        let _ = cr.fill();
    }
    if look.error {
        let ez = (disc * 0.28).round().max(8.0);
        let (ex, ey) = (side - width - ez / 2.0, side - width - ez / 2.0);
        rgb(cr, GROUND, 1.0);
        cr.new_sub_path();
        cr.arc(ex, ey, ez / 2.0 + 2.0, 0.0, TAU);
        let _ = cr.fill();
        rgb(cr, VERMILION, 1.0);
        cr.new_sub_path();
        cr.arc(ex, ey, ez / 2.0, 0.0, TAU);
        let _ = cr.fill();
    }
}
