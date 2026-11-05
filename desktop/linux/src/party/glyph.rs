//! The four reactions as one drawn set: monochrome, round-capped strokes on a 24 unit grid,
//! drawn in the widget's own text colour at the weight the header buttons use. Adwaita has no
//! clap or crying face, so the set is drawn here and the same four are drawn on every platform.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use adw::prelude::*;
use kaeri_model::party::Reaction;

/// The stroke weight of the header buttons' symbolic icons, on the 24 unit grid.
pub const WEIGHT: f64 = 1.7;

/// The four, in the order they are laid out.
pub const ALL: [Reaction; 4] = [Reaction::Laugh, Reaction::Gasp, Reaction::Clap, Reaction::Cry];

/// A drawing area `size` px square showing `reaction` in its text colour. It follows the
/// colour of the widget it sits in, so a flat button's hover and a dimmed button colour it.
pub fn icon(reaction: Reaction, size: i32, weight: f64) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_width(size);
    area.set_content_height(size);
    area.set_halign(gtk::Align::Center);
    area.set_valign(gtk::Align::Center);
    area.set_can_target(false);
    area.set_accessible_role(gtk::AccessibleRole::Presentation);
    area.set_draw_func(move |area, cr, width, height| {
        let color = area.color();
        cr.set_source_rgba(
            f64::from(color.red()),
            f64::from(color.green()),
            f64::from(color.blue()),
            f64::from(color.alpha()),
        );
        let side = f64::from(width.min(height));
        cr.translate((f64::from(width) - side) / 2.0, (f64::from(height) - side) / 2.0);
        draw(cr, reaction, side, weight);
    });
    area
}

/// `reaction` drawn in the current source colour, in a square `side` px wide.
pub fn draw(cr: &gtk::cairo::Context, reaction: Reaction, side: f64, weight: f64) {
    let _ = cr.save();
    cr.scale(side / 24.0, side / 24.0);
    cr.set_line_width(weight);
    cr.set_line_cap(gtk::cairo::LineCap::Round);
    cr.set_line_join(gtk::cairo::LineJoin::Round);
    let face = |cr: &gtk::cairo::Context| {
        cr.new_sub_path();
        cr.arc(12.0, 12.0, 9.0, 0.0, TAU);
        let _ = cr.stroke();
    };
    let dot = |cr: &gtk::cairo::Context, x: f64, y: f64, r: f64| {
        cr.new_sub_path();
        cr.arc(x, y, r, 0.0, TAU);
        let _ = cr.fill();
    };
    match reaction {
        Reaction::Laugh => {
            face(cr);
            for x in [8.0, 13.0] {
                cr.move_to(x, 10.3);
                quad(cr, (x, 10.3), (x + 1.5, 8.4), (x + 3.0, 10.3));
            }
            let _ = cr.stroke();
            cr.new_sub_path();
            cr.move_to(7.6, 13.4);
            cr.line_to(16.4, 13.4);
            cr.arc(12.0, 13.4, 4.4, 0.0, PI);
            cr.close_path();
            let _ = cr.stroke();
        }
        Reaction::Gasp => {
            face(cr);
            dot(cr, 9.0, 10.0, 1.0);
            dot(cr, 15.0, 10.0, 1.0);
            cr.new_sub_path();
            cr.arc(12.0, 15.3, 2.2, 0.0, TAU);
            let _ = cr.stroke();
        }
        Reaction::Clap => {
            for (x, angle, pivot) in [(5.6, -16.0_f64, 8.5), (12.6, 16.0, 15.5)] {
                let _ = cr.save();
                cr.translate(pivot, 14.6);
                cr.rotate(angle.to_radians());
                cr.translate(-pivot, -14.6);
                rounded(cr, x, 8.6, 5.8, 12.0, 2.9);
                let _ = cr.stroke();
                let _ = cr.restore();
            }
            for (from, to) in [
                ((12.0, 2.6), (12.0, 4.9)),
                ((7.4, 3.8), (8.6, 5.7)),
                ((16.6, 3.8), (15.4, 5.7)),
            ] {
                cr.move_to(from.0, from.1);
                cr.line_to(to.0, to.1);
            }
            let _ = cr.stroke();
        }
        Reaction::Cry => {
            face(cr);
            cr.move_to(8.0, 10.0);
            cr.line_to(10.5, 10.0);
            cr.move_to(13.5, 10.0);
            cr.line_to(16.0, 10.0);
            cr.move_to(9.0, 16.6);
            quad(cr, (9.0, 16.6), (12.0, 14.3), (15.0, 16.6));
            let _ = cr.stroke();
            // The tear, a filled drop.
            cr.new_sub_path();
            cr.move_to(16.2, 11.7);
            cr.curve_to(16.2, 11.7, 15.0, 13.5, 15.0, 14.3);
            cr.arc_negative(16.2, 14.3, 1.2, PI, 0.0);
            cr.curve_to(17.4, 13.5, 16.2, 11.7, 16.2, 11.7);
            cr.close_path();
            let _ = cr.fill();
        }
    }
    let _ = cr.restore();
}

/// A quadratic curve from `from`, which is where the pen already is, as cairo's cubic.
fn quad(cr: &gtk::cairo::Context, from: (f64, f64), via: (f64, f64), to: (f64, f64)) {
    cr.curve_to(
        from.0 + 2.0 / 3.0 * (via.0 - from.0),
        from.1 + 2.0 / 3.0 * (via.1 - from.1),
        to.0 + 2.0 / 3.0 * (via.0 - to.0),
        to.1 + 2.0 / 3.0 * (via.1 - to.1),
        to.0,
        to.1,
    );
}

fn rounded(cr: &gtk::cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -FRAC_PI_2, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, FRAC_PI_2);
    cr.arc(x + r, y + h - r, r, FRAC_PI_2, PI);
    cr.arc(x + r, y + r, r, PI, PI + FRAC_PI_2);
    cr.close_path();
}
