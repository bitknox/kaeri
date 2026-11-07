//! Library, Reading side: the reading lists, a segmented control over a grid of covers.

use std::{cell::Cell, rc::Rc};

use adw::prelude::*;
use kaeri_i18n::t;
use kaeri_model::{
    event::Event,
    reading::{LibrarySegmentRead as Segment, ReadingCommand, ReadingEvent},
};

use super::{Dest, WorkGrid, listen, scope, send};
use crate::{
    nagare,
    states::{Shape, States},
    widgets,
    window::Ctx,
};

const SEGMENTS: [Segment; 7] = [
    Segment::Reading,
    Segment::Planning,
    Segment::Completed,
    Segment::Paused,
    Segment::Dropped,
    Segment::Rereading,
    Segment::YourFiles,
];

pub fn name(segment: Segment) -> String {
    match segment {
        Segment::Reading => t::read_list_reading(),
        Segment::Planning => t::read_list_planning(),
        Segment::Completed => t::read_list_completed(),
        Segment::Paused => t::read_list_paused(),
        Segment::Dropped => t::read_list_dropped(),
        Segment::Rereading => t::read_list_rereading(),
        Segment::YourFiles => t::read_list_your_files(),
    }
}

pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let grid = WorkGrid::new(&ctx.hub);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&grid.widget)
        .build();
    let empty = nagare::status_page(
        &t::read_library_files_empty_title(),
        &t::read_library_files_empty_line(),
        None,
    );
    let states = States::new(&ctx.hub, &scroll, Shape::Grid, &empty, &t::error_generic());

    let toggles = adw::ToggleGroup::new();
    for segment in SEGMENTS {
        toggles.add(adw::Toggle::builder().name(segment.as_str()).label(name(segment)).build());
    }
    toggles.add_css_class("round");
    toggles.set_active_name(Some(Segment::Reading.as_str()));
    let current = Rc::new(Cell::new(Segment::Reading));
    let ask = {
        let hub = Rc::downgrade(&ctx.hub);
        let states = states.clone();
        move |segment: Segment| {
            if let Some(hub) = hub.upgrade() {
                states.loading(None);
                send(&hub, ReadingCommand::LoadLibrary { segment });
            }
        }
    };
    toggles.connect_active_name_notify({
        let (current, ask) = (Rc::clone(&current), ask.clone());
        move |toggles| {
            if let Some(segment) = toggles.active_name().and_then(|n| Segment::parse(&n)) {
                current.set(segment);
                ask(segment);
            }
        }
    });

    let header = adw::HeaderBar::new();
    header.pack_start(&widgets::sidebar_button(&ctx.split));
    header.pack_end(&scope(ctx, Dest::Library));
    header.set_title_widget(Some(&toggles));
    let page = widgets::content_page(&t::nav_library(), &header, &states.widget);
    states.on_retry({
        let (current, ask) = (Rc::clone(&current), ask.clone());
        move || ask(current.get())
    });

    let grid = Rc::new(grid);
    listen(ctx, &page, {
        let (states, grid, current, ask) =
            (states.clone(), Rc::clone(&grid), Rc::clone(&current), ask.clone());
        let hub = Rc::downgrade(&ctx.hub);
        move |event| match event {
            Event::Reading(ReadingEvent::Library { segment, tiles })
                if *segment == current.get() =>
            {
                grid.set(tiles);
                if tiles.is_empty() {
                    states.empty();
                } else {
                    states.content();
                }
            }
            Event::Medium(_) if hub.upgrade().is_some_and(|h| super::reads(&h)) => {
                ask(current.get());
            }
            _ => {}
        }
    });
    if super::reads(&ctx.hub) {
        ask(current.get());
    }
    page
}
