//! Read, reading's Home: the spotlight, then shelves of covers (continue, this week, up next,
//! releasing now, because you finished, a genre). Drawn from what the engine says; a shelf with
//! nothing in it is not drawn.

use std::rc::Rc;

use adw::prelude::*;
use kaeri_i18n::t;
use kaeri_model::{
    event::Event,
    medium::KaeriIsFor,
    reading::{ReadSpotlightCase, ReadingCommand, ReadingEvent, ReadingHomeView, WorkTile},
};

use super::{
    COVER_WIDTH, Place, action_chapter, cover, heading_row, listen, place, read_button, send,
    shelf, work_card,
};
use crate::{say, states::Shape, widgets, window::Ctx};

pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let empty = crate::states::plain(&t::home_read_end(), &t::home_read_no_sources());
    let Place { page, states, column, .. } = place(
        ctx,
        &super::title_of(crate::pages::Dest::Read),
        Shape::Grid,
        &empty,
        &t::error_generic(),
        true,
    );
    states.loading(None);
    states.on_retry({
        let hub = Rc::downgrade(&ctx.hub);
        move || {
            if let Some(hub) = hub.upgrade() {
                send(&hub, ReadingCommand::LoadHome { refresh: true });
            }
        }
    });
    listen(ctx, &page, {
        let (ctx, states, column) = (Rc::downgrade(ctx), states.clone(), column.clone());
        let page = page.downgrade();
        move |event| {
            let Some(ctx) = ctx.upgrade() else { return };
            match event {
                Event::Reading(ReadingEvent::Home(view)) => {
                    if view.failed && super::is_blank(view) {
                        states.failed("");
                        return;
                    }
                    super::clear(&column);
                    draw(&ctx, &column, view);
                    if super::is_blank(view) {
                        states.empty();
                    } else {
                        states.content();
                    }
                }
                Event::Medium(_) if super::reads(&ctx.hub) => {
                    // Read beside Home when both are on, Home when reading is all there is.
                    if let Some(page) = page.upgrade() {
                        page.set_title(&if super::answer(&ctx.hub) == KaeriIsFor::Both {
                            t::nav_read()
                        } else {
                            t::home_title()
                        });
                    }
                    send(&ctx.hub, ReadingCommand::LoadHome { refresh: false });
                }
                _ => {}
            }
        }
    });
    if super::reads(&ctx.hub) {
        send(&ctx.hub, ReadingCommand::LoadHome { refresh: false });
    }
    page.set_tag(Some("read"));
    page
}

fn draw(ctx: &Rc<Ctx>, column: &gtk::Box, view: &ReadingHomeView) {
    if let Some(spot) = &view.spot
        && let Some(tile) = &view.spotlight
    {
        column.append(&spotlight(ctx, tile, spot));
    }
    let mut shelves: Vec<(String, &[WorkTile])> = vec![
        (t::home_continue(), &view.continue_reading),
        (t::home_this_week(), &view.this_week),
        (t::home_up_next(), &view.up_next),
        (t::home_read_releasing(), &view.releasing),
    ];
    if let Some(title) = &view.because_title {
        shelves.push((t::home_because(title), &view.because));
    }
    if let Some(genre) = &view.genre {
        shelves.push((genre.clone(), &view.genre_shelf));
    }
    for (title, tiles) in shelves {
        if tiles.is_empty() {
            continue;
        }
        column.append(&heading_row(&title, None, None));
        let cards: Vec<gtk::Widget> =
            tiles.iter().map(|tile| work_card(&ctx.hub, tile, COVER_WIDTH).upcast()).collect();
        column.append(&shelf(&cards, 14));
    }
    match view.end {
        Some(kaeri_model::reading::ReadHomeEnd::Everything) => {
            column.append(&widgets::haze(&t::home_read_end(), "caption"));
        }
        Some(kaeri_model::reading::ReadHomeEnd::NoSources) => {
            column.append(&widgets::haze(&t::home_read_no_sources(), "caption"));
        }
        None => {}
    }
}

fn spotlight(
    ctx: &Rc<Ctx>,
    tile: &WorkTile,
    spot: &kaeri_model::reading::ReadSpotlight,
) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Horizontal, 20);
    card.add_css_class("read-spotlight");
    card.append(&cover(&ctx.hub, tile.cover.as_deref(), &tile.title, 160));
    let text = gtk::Box::new(gtk::Orientation::Vertical, 6);
    text.set_valign(gtk::Align::Center);
    text.set_hexpand(true);
    let label = widgets::haze(&say::msg(&spot.label), "caption-heading");
    label.set_xalign(0.0);
    text.append(&label);
    text.append(&widgets::show_title(&tile.title, "home-spot-title"));
    let facts = super::dotted(&spot.facts);
    if !facts.is_empty() {
        text.append(&widgets::haze(&facts, "caption"));
    }
    if let Some(line) = &spot.line {
        text.append(&widgets::haze(&say::msg(line), "caption"));
    }
    if let Some(synopsis) = &spot.synopsis {
        let body = gtk::Label::new(Some(synopsis));
        body.set_wrap(true);
        body.set_lines(3);
        body.set_ellipsize(gtk::pango::EllipsizeMode::End);
        body.set_xalign(0.0);
        body.set_max_width_chars(70);
        text.append(&body);
    }
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_margin_top(6);
    if let Some((chapter, label)) = spot.action.as_ref().and_then(action_chapter) {
        buttons.append(&read_button(&label, &tile.work, &chapter));
    }
    let open = gtk::Button::with_label(&t::chapter_updates_open_series_menu());
    open.add_css_class("pill");
    open.set_action_target_value(Some(&tile.work.0.to_variant()));
    open.set_action_name(Some("win.open-series"));
    buttons.append(&open);
    text.append(&buttons);
    card.append(&text);
    let _ = ReadSpotlightCase::Popular;
    card
}
