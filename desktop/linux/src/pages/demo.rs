//! The built-in demo where it shows as a work: the credit block with its three links, shown
//! wherever the work shows as a title, as its licence asks.

use std::rc::Rc;

use adw::prelude::*;
use kaeri_i18n::t;
use kaeri_model::sources::DemoInfo;

use crate::{widgets, window::Ctx};

/// The credit line and its links: Licence, Wikimedia Commons and the makers.
pub fn credit(ctx: &Rc<Ctx>, info: &DemoInfo) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let line = widgets::haze(&info.credit, "caption");
    column.append(&line);
    let links = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    for (label, url) in [
        (t::sources_demo_link_licence(), &info.licence_url),
        (t::sources_demo_link_commons(), &info.file_page_url),
        (t::sources_demo_link_maker(), &info.maker_url),
    ] {
        let link = gtk::LinkButton::with_label(url, &label);
        link.set_halign(gtk::Align::Start);
        link.add_css_class("flat");
        link.connect_activate_link({
            let (weak, url) = (Rc::downgrade(ctx), url.clone());
            move |_| {
                if let Some(ctx) = weak.upgrade() {
                    crate::portal::open_uri(&ctx.window, &url, || {});
                }
                gtk::glib::Propagation::Stop
            }
        });
        links.append(&link);
    }
    column.append(&links);
    column
}
