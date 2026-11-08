//! Read, reading's Home: the spotlight, the person's own rows (Continue, This week, Up next), then
//! the discovery rows the engine picks (Trending, New, Finished, Because you finished, More of a
//! genre), then the closing line. Every row is posters at 128 by 192, 16 apart, 36 between rows.
//!
//! A discovery row keeps its slot across sends: one still on its way shows nothing for 600 ms,
//! then placeholders at final size under its real heading, held at least 0.3 s, and its covers
//! crossfade in over them. A row that fails with nothing kept fades and its gap closes.

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

use adw::prelude::*;
use gtk::glib;
use kaeri_i18n::t;
use kaeri_model::{
    event::Event,
    medium::KaeriIsFor,
    reading::{
        ReadHomeEnd, ReadHomeRow, ReadRowKind, ReadSearchQuery, ReadingCommand, ReadingEvent,
        ReadingHomeView, WorkTile,
    },
};

use super::{
    Place, action_chapter, cover,
    discovery::{self, Landing},
    listen, place, read_button, send,
};
use crate::{motion, pages::Dest, say, squeeze::Squeeze, states::Shape, widgets, window::Ctx};

/// Between one row and the next.
const BETWEEN: i32 = 36;
/// Over the closing line.
const ABOVE_END: i32 = 28;
/// Between the demo's spotlight and the line under it.
const DEMO_LINE: i32 = 20;

/// One discovery row's slot.
struct RowSlot {
    id: String,
    /// What it shows, to leave it be when a send changes nothing in it.
    signature: RefCell<String>,
    place: Squeeze,
    inner: gtk::Box,
    title: gtk::Label,
    side: gtk::Label,
    all: gtk::Button,
    landing: Rc<Landing>,
    query: Rc<RefCell<Option<ReadSearchQuery>>>,
}

struct Home {
    ctx: Weak<Ctx>,
    own: gtk::Box,
    own_signature: RefCell<String>,
    rows: gtk::Box,
    end: gtk::Box,
    end_signature: RefCell<String>,
    slots: RefCell<Vec<Rc<RowSlot>>>,
    /// The failure notice has been shown for this failure.
    told: Cell<bool>,
    /// Something has been drawn: what lands whole after that fades in.
    drawn: Cell<bool>,
}

pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let empty = crate::states::plain(&t::home_read_end(), &t::home_read_no_sources());
    let Place { page, states, column, .. } = place(
        ctx,
        &super::title_of(Dest::Read),
        Shape::Grid,
        &empty,
        &t::error_generic(),
        true,
    );
    column.set_spacing(0);
    let own = gtk::Box::new(gtk::Orientation::Vertical, BETWEEN);
    let rows = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let end = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.append(&own);
    column.append(&rows);
    column.append(&end);
    let home = Rc::new(Home {
        ctx: Rc::downgrade(ctx),
        own,
        own_signature: RefCell::default(),
        rows,
        end,
        end_signature: RefCell::default(),
        slots: RefCell::default(),
        told: Cell::new(false),
        drawn: Cell::new(false),
    });
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
        let (ctx, states) = (Rc::downgrade(ctx), states.clone());
        let page = page.downgrade();
        let home = Rc::clone(&home);
        move |event| {
            let Some(ctx) = ctx.upgrade() else { return };
            match event {
                Event::Reading(ReadingEvent::Home(view)) => {
                    if view.failed && super::is_blank(view) {
                        states.failed("");
                        return;
                    }
                    home.draw(&ctx, view);
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

impl Home {
    fn draw(self: &Rc<Self>, ctx: &Rc<Ctx>, view: &ReadingHomeView) {
        let demo = ctx.hub.known.borrow().demo.only_books;
        self.draw_own(ctx, view, demo);
        self.draw_rows(ctx, view);
        self.draw_end(view, demo);
        self.drawn.set(true);
        // Couldn't load suggestions, once a failure, with Try again.
        if view.failed {
            if !self.told.replace(true) {
                ctx.toast_with(&t::home_failed(), &t::home_try_again(), "win.read-home-reload");
            }
        } else {
            self.told.set(false);
        }
    }

    /// The spotlight and the person's own rows, drawn again only when they changed.
    fn draw_own(&self, ctx: &Rc<Ctx>, view: &ReadingHomeView, demo: bool) {
        let signature = format!(
            "{demo}{:?}{:?}{:?}{:?}{:?}", // i18n-skip
            view.spotlight, view.spot, view.continue_reading, view.this_week, view.up_next
        );
        if *self.own_signature.borrow() == signature {
            return;
        }
        self.own_signature.replace(signature);
        super::clear(&self.own);
        if demo {
            let top = gtk::Box::new(gtk::Orientation::Vertical, DEMO_LINE);
            if let Some(card) = super::demo::spotlight(ctx) {
                top.append(&card);
            }
            top.append(&demo_line());
            self.own.append(&top);
        } else if let Some(spot) = &view.spot
            && let Some(tile) = &view.spotlight
        {
            self.own.append(&spotlight(ctx, tile, spot));
        }
        for (title, tiles) in [
            (t::home_continue(), &view.continue_reading),
            (t::home_this_week(), &view.this_week),
            (t::home_up_next(), &view.up_next),
        ] {
            if tiles.is_empty() {
                continue;
            }
            let section = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let (head, _, _, _) = heading(&title);
            section.append(&head);
            section.append(&discovery::strip(&posters(ctx, tiles), &title));
            self.own.append(&section);
        }
        self.first_gap();
    }

    /// The discovery rows: each keeps its slot by id, lands with the crossfade, and leaves by
    /// fading with its gap closing after it.
    fn draw_rows(self: &Rc<Self>, ctx: &Rc<Ctx>, view: &ReadingHomeView) {
        let mut old = self.slots.take();
        let mut live: Vec<Rc<RowSlot>> = Vec::new();
        for row in &view.rows {
            let slot = match old.iter().position(|s| s.id == row.id) {
                Some(at) => old.remove(at),
                None => self.new_slot(ctx, row),
            };
            self.fill(ctx, &slot, row);
            live.push(slot);
        }
        // Rows that left: those on show fade and close their gap; the rest go at once.
        for slot in old {
            if slot.place.is_closing() {
                continue;
            }
            if slot.place.is_visible() && slot.place.is_mapped() {
                slot.place.mark_closing();
                let (rows, place) = (self.rows.clone(), slot.place.clone());
                motion::fade_away(&slot.inner, move || {
                    let (rows, gone) = (rows.clone(), place.clone());
                    place.close(move || {
                        if gone.parent().is_some() {
                            rows.remove(&gone);
                        }
                    });
                });
            } else {
                self.rows.remove(&slot.place);
            }
        }
        let mut previous: Option<gtk::Widget> = None;
        for slot in &live {
            if slot.place.parent().as_ref() == Some(self.rows.upcast_ref()) {
                self.rows.reorder_child_after(&slot.place, previous.as_ref());
            } else {
                self.rows.insert_child_after(&slot.place, previous.as_ref());
            }
            previous = Some(slot.place.clone().upcast());
        }
        self.slots.replace(live);
        self.first_gap();
    }

    fn new_slot(self: &Rc<Self>, ctx: &Rc<Ctx>, row: &ReadHomeRow) -> Rc<RowSlot> {
        let inner = gtk::Box::new(gtk::Orientation::Vertical, 0);
        inner.set_margin_top(BETWEEN);
        let (head, title, side, all) = heading("");
        let query: Rc<RefCell<Option<ReadSearchQuery>>> = Rc::default();
        all.connect_clicked({
            let (ctx, query) = (Rc::downgrade(ctx), Rc::clone(&query));
            move |_| {
                if let (Some(ctx), Some(query)) = (ctx.upgrade(), query.borrow().clone()) {
                    view_all(&ctx, query);
                }
            }
        });
        inner.append(&head);
        let landing = Landing::new(discovery::row_placeholders);
        inner.append(&landing.widget);
        let place = Squeeze::new(&inner, gtk::Orientation::Vertical, 0);
        place.set_visible(false);
        landing.on_visible({
            let (place, home) = (place.clone(), Rc::downgrade(self));
            move || {
                place.set_visible(true);
                if let Some(home) = home.upgrade() {
                    home.first_gap();
                }
            }
        });
        Rc::new(RowSlot {
            id: row.id.clone(),
            signature: RefCell::default(),
            place,
            inner,
            title,
            side,
            all,
            landing,
            query,
        })
    }

    /// The row's heading and covers as the engine sent them.
    fn fill(&self, ctx: &Rc<Ctx>, slot: &Rc<RowSlot>, row: &ReadHomeRow) {
        let title = say::msg(&row.title);
        slot.title.set_label(&title);
        let side = row.side.as_ref().map(say::msg);
        slot.side.set_label(side.as_deref().unwrap_or_default());
        slot.side.set_visible(side.is_some());
        slot.query.replace(row.view_all.clone());
        slot.all.set_visible(row.view_all.is_some() && row.kind != ReadRowKind::Because);
        slot.all.update_property(&[gtk::accessible::Property::Label(
            &t::read_linux_view_all_row(&title),
        )]);
        if row.pending {
            slot.title.update_property(&[gtk::accessible::Property::Label(
                &t::read_linux_row_loading(&title),
            )]);
            slot.landing.wait(true);
            return;
        }
        slot.title.update_property(&[gtk::accessible::Property::Label(&title)]);
        let signature = format!("{:?}", row.tiles);
        if *slot.signature.borrow() == signature && slot.landing.is_visible() {
            return;
        }
        slot.signature.replace(signature);
        let strip = discovery::strip(&posters(ctx, &row.tiles), &title);
        let fresh = !slot.landing.is_visible() && !slot.landing.is_waiting();
        slot.landing.land(&strip, false);
        // A row that lands whole after Home was drawn, with no placeholders before it, fades in.
        if fresh && self.drawn.get() {
            let inner = slot.inner.clone();
            motion::once_mapped(&slot.inner, move || {
                motion::fade_in(&inner);
            });
        }
    }

    /// The first thing on the page starts at the top: a row with nothing above it takes no gap.
    fn first_gap(&self) {
        let mut above = self.own.first_child().is_some();
        for slot in self.slots.borrow().iter() {
            slot.inner.set_margin_top(if above { BETWEEN } else { 0 });
            above |= slot.place.is_visible();
        }
    }

    /// The closing line under a faint rule, once no row is still on its way; or, on a first run
    /// with no connection and nothing kept, the line saying the rows come once online.
    fn draw_end(&self, view: &ReadingHomeView, demo: bool) {
        let waiting = view.rows.iter().any(|r| r.pending);
        let offline_rows = view.rows.is_empty() && view.offline;
        let signature = format!("{waiting}{offline_rows}{:?}{demo}", view.end); // i18n-skip
        if *self.end_signature.borrow() == signature {
            return;
        }
        self.end_signature.replace(signature);
        super::clear(&self.end);
        if waiting {
            return;
        }
        let full_lists =
            || vec![(t::nav_updates(), "updates"), (t::nav_search(), "read-search")];
        let words = if offline_rows {
            Some((t::read_home_offline_rows(), Vec::new()))
        } else {
            match view.end {
                Some(ReadHomeEnd::Everything) => Some((t::home_read_end(), full_lists())),
                // The demo's line already offers a source, under its spotlight.
                Some(ReadHomeEnd::NoSources) if demo => {
                    Some((t::home_read_end(), full_lists()))
                }
                Some(ReadHomeEnd::NoSources) => Some((t::home_read_no_sources(), Vec::new())),
                None => None,
            }
        };
        let Some((text, links)) = words else { return };
        let column = gtk::Box::new(gtk::Orientation::Vertical, 16);
        column.set_margin_top(ABOVE_END);
        let rule = gtk::Separator::new(gtk::Orientation::Horizontal);
        rule.add_css_class("home-rule");
        column.append(&rule);
        let label = gtk::Label::new(None);
        label.set_markup(&linked(&text, &links));
        label.add_css_class("haze");
        label.add_css_class("home-end");
        label.set_xalign(0.0);
        label.set_wrap(true);
        label.connect_activate_link({
            let ctx = self.ctx.clone();
            move |label, uri| {
                // Search opens on the Reading side.
                if uri == Dest::ReadSearch.name()
                    && let Some(ctx) = ctx.upgrade()
                {
                    ctx.prefs.set_scope_reading(Dest::Search.name(), true);
                }
                let _ = WidgetExt::activate_action(label, "win.go", Some(&uri.to_variant()));
                glib::Propagation::Stop
            }
        });
        column.append(&label);
        self.end.append(&column);
        if self.drawn.get() {
            let shown = column.clone();
            motion::once_mapped(&column, move || {
                motion::fade_in(&shown);
            });
        }
    }
}

/// View all: Search on the Reading side, with the row's exact filters and order.
fn view_all(ctx: &Rc<Ctx>, query: ReadSearchQuery) {
    ctx.prefs.set_scope_reading(Dest::Search.name(), true);
    let open = ctx.read_view_all.borrow().clone();
    if let Some(open) = open {
        open(query);
    }
    let _ = WidgetExt::activate_action(
        &ctx.window,
        "win.go",
        Some(&Dest::ReadSearch.name().to_variant()),
    );
}

/// Posters for `tiles`.
fn posters(ctx: &Rc<Ctx>, tiles: &[WorkTile]) -> Vec<gtk::Button> {
    tiles.iter().map(|tile| discovery::poster(&ctx.hub, tile, None)).collect()
}

/// A row's heading: its title (a heading for screen readers), the side text and View all.
fn heading(title: &str) -> (gtk::Box, gtk::Label, gtk::Label, gtk::Button) {
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    head.set_margin_bottom(12);
    let label = gtk::Label::new(Some(title));
    label.add_css_class("home-heading");
    label.set_xalign(0.0);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.set_accessible_role(gtk::AccessibleRole::Heading);
    head.append(&label);
    let aside = gtk::Label::new(None);
    aside.add_css_class("haze");
    aside.add_css_class("read-row-side");
    aside.set_valign(gtk::Align::Center);
    aside.set_visible(false);
    head.append(&aside);
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    head.append(&spacer);
    let words = gtk::Label::new(Some(&t::home_view_all()));
    let chevron = gtk::Image::from_icon_name("go-next-symbolic");
    chevron.set_pixel_size(12);
    let inner = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    inner.append(&words);
    inner.append(&chevron);
    let all = gtk::Button::new();
    all.set_child(Some(&inner));
    all.add_css_class("flat");
    all.add_css_class("home-all");
    all.set_valign(gtk::Align::Center);
    all.set_visible(false);
    head.append(&all);
    (head, label, aside, all)
}

/// `text` as markup, the first time each name in `links` appears made a link to its place.
fn linked(text: &str, links: &[(String, &str)]) -> String {
    let mut markup = glib::markup_escape_text(text).to_string();
    for (name, dest) in links {
        let name = glib::markup_escape_text(name).to_string();
        if let Some(at) = markup.find(&name) {
            markup
                .replace_range(at..at + name.len(), &format!("<a href=\"{dest}\">{name}</a>")); // i18n-skip
        }
    }
    markup
}

/// Under the demo's spotlight: that only the demo is on, and Add a source.
fn demo_line() -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    let words = widgets::haze(&t::home_demo_only_books(), "read-demo-line");
    words.set_hexpand(true);
    words.set_valign(gtk::Align::Center);
    row.append(&words);
    let add = gtk::Button::with_label(&t::sources_add_title());
    add.add_css_class("home-sec");
    add.add_css_class("home-small");
    add.set_valign(gtk::Align::Center);
    add.connect_clicked(super::demo::add_a_source);
    row.append(&add);
    row
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
    card
}
