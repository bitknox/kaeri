//! Search, Reading side: a field over a grid of covers, more on request.

use std::{cell::RefCell, rc::Rc, time::Duration};

use adw::prelude::*;
use gtk::glib;
use kaeri_i18n::t;
use kaeri_model::{
    event::Event,
    reading::{ReadSearchQuery, ReadingCommand, ReadingEvent},
};

use super::{Dest, WorkGrid, listen, scope, send};
use crate::{
    nagare,
    states::{Shape, States},
    widgets,
    window::Ctx,
};

fn query(text: &str) -> ReadSearchQuery {
    ReadSearchQuery {
        text: text.trim().to_owned(),
        medium: None,
        formats: Vec::new(),
        origins: Vec::new(),
        status: None,
        genres: Vec::new(),
        exclude_genres: Vec::new(),
        sort: None,
    }
}

pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let grid = Rc::new(WorkGrid::new(&ctx.hub));
    let more = gtk::Button::with_label(&t::action_more());
    more.add_css_class("pill");
    more.set_halign(gtk::Align::Center);
    more.set_margin_top(8);
    more.set_margin_bottom(16);
    more.set_visible(false);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.append(&grid.widget);
    body.append(&more);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&body)
        .build();
    let empty = nagare::status_page(
        &t::read_search_nothing_browse(),
        &t::read_search_nothing_line(),
        None,
    );
    let states = States::new(&ctx.hub, &scroll, Shape::Grid, &empty, &t::read_search_failed());

    let entry = gtk::SearchEntry::builder()
        .placeholder_text(t::read_search_placeholder())
        .hexpand(true)
        .build();
    let clamp = adw::Clamp::builder().maximum_size(520).child(&entry).build();
    let header = adw::HeaderBar::new();
    header.pack_start(&widgets::sidebar_button(&ctx.split));
    header.pack_end(&scope(ctx, Dest::Search));
    header.set_title_widget(Some(&clamp));
    let page = widgets::content_page(&t::nav_search(), &header, &states.widget);

    let at = Rc::new(RefCell::new((String::new(), 1_u32)));
    let ask = {
        let hub = Rc::downgrade(&ctx.hub);
        let states = states.clone();
        let at = Rc::clone(&at);
        move |text: String| {
            if let Some(hub) = hub.upgrade() {
                *at.borrow_mut() = (text.clone(), 1);
                states.loading(None);
                send(&hub, ReadingCommand::Search { query: query(&text) });
            }
        }
    };
    let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::default();
    entry.connect_search_changed({
        let (ask, pending) = (ask.clone(), Rc::clone(&pending));
        move |entry| {
            if let Some(id) = pending.borrow_mut().take() {
                id.remove();
            }
            let text = entry.text().to_string();
            let (ask, slot) = (ask.clone(), Rc::clone(&pending));
            let id = glib::timeout_add_local_once(Duration::from_millis(350), move || {
                slot.borrow_mut().take();
                ask(text);
            });
            *pending.borrow_mut() = Some(id);
        }
    });
    more.connect_clicked({
        let (hub, at) = (Rc::downgrade(&ctx.hub), Rc::clone(&at));
        move |_| {
            if let Some(hub) = hub.upgrade() {
                let (text, page) = at.borrow().clone();
                send(&hub, ReadingCommand::SearchMore { query: query(&text), page: page + 1 });
            }
        }
    });
    states.on_retry({
        let (ask, at) = (ask.clone(), Rc::clone(&at));
        move || ask(at.borrow().0.clone())
    });
    let tiles = Rc::new(RefCell::new(Vec::new()));
    listen(ctx, &page, {
        let (states, grid, more, at, tiles) =
            (states.clone(), Rc::clone(&grid), more.clone(), Rc::clone(&at), Rc::clone(&tiles));
        move |event| match event {
            Event::Reading(ReadingEvent::Search { page, tiles: found, has_more, .. }) => {
                if *page <= 1 {
                    tiles.borrow_mut().clear();
                }
                at.borrow_mut().1 = *page;
                tiles.borrow_mut().extend(found.iter().cloned());
                grid.set(&tiles.borrow());
                more.set_visible(*has_more);
                if tiles.borrow().is_empty() {
                    states.empty();
                } else {
                    states.content();
                }
            }
            Event::Reading(ReadingEvent::Failed {
                about: kaeri_model::reading::ReadingAbout::Search,
                failure,
            }) => {
                states.failed(&super::failure(failure));
            }
            _ => {}
        }
    });
    if super::reads(&ctx.hub) {
        ask(String::new());
    }
    page.set_tag(Some("read-search"));
    page
}
