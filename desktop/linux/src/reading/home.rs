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
use kaeri_core::books::WorkId;
use kaeri_i18n::t;
use kaeri_model::{
    event::Event,
    medium::KaeriIsFor,
    reading::{
        ReadHomeEnd, ReadHomePart, ReadHomeRow, ReadRowKind, ReadSearchQuery, ReadingCommand,
        ReadingEvent, ReadingHomeView, WorkTile,
    },
};

use super::{
    Place, action_chapter, cover,
    discovery::{self, Caption, Landing, Part},
    listen, place, read_button, send,
};
use crate::{
    motion,
    pages::Dest,
    say,
    states::Shape,
    widgets,
    window::{Ctx, Size},
};

/// Between one row and the next.
const BETWEEN: i32 = 36;
/// Over the closing line.
const ABOVE_END: i32 = 28;
/// Between the demo's spotlight and the line under it.
const DEMO_LINE: i32 = 20;
/// Rows still on their way this long put Loading suggestions in the header.
const ACTIVITY: std::time::Duration = std::time::Duration::from_secs(4);

/// One discovery row's slot.
struct RowSlot {
    id: String,
    /// What it shows, to leave it be when a send changes nothing in it.
    signature: RefCell<String>,
    part: Rc<Part>,
    title: gtk::Label,
    side: gtk::Label,
    all: gtk::Button,
    /// View all's query and the line over its results.
    query: Rc<RefCell<Option<ViewAll>>>,
}

/// The spotlight's or one of the person's own sections' slot.
struct OwnSlot {
    part: Rc<Part>,
    title: String,
    signature: RefCell<String>,
}

struct Home {
    ctx: Weak<Ctx>,
    own: gtk::Box,
    /// The spotlight, then Continue, This week and Up next for you.
    own_slots: RefCell<Vec<OwnSlot>>,
    rows: gtk::Box,
    end: gtk::Box,
    end_signature: RefCell<String>,
    slots: RefCell<Vec<Rc<RowSlot>>>,
    /// The failure notice has been shown for this failure.
    told: Cell<bool>,
    /// Something has been drawn: what lands whole after that fades in.
    drawn: Cell<bool>,
    /// Loading suggestions, in the header once rows have waited 4 s.
    capsule: gtk::Box,
    capsule_spin: gtk::Spinner,
    activity: RefCell<Option<glib::SourceId>>,
    /// The spotlight's card and its actions, and its placeholder's, laid out for the window.
    spots: RefCell<Vec<Spot>>,
}

/// A spotlight card and its row of actions; a placeholder has no actions.
type Spot = (glib::WeakRef<gtk::Box>, Option<glib::WeakRef<gtk::Box>>);

pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let empty = crate::states::plain(&t::home_read_end(), &t::home_read_no_sources());
    let Place { page, header, states, column } = place(
        ctx,
        &super::title_of(Dest::Read),
        Shape::Grid,
        &empty,
        &t::error_generic(),
        true,
    );
    column.set_spacing(0);
    // As on Watch's Home: the header says suggestions are still loading, and nothing else.
    let capsule = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    capsule.add_css_class("home-capsule");
    let capsule_spin = gtk::Spinner::new();
    let capsule_text = gtk::Label::new(Some(&t::home_loading()));
    capsule_text.add_css_class("caption");
    capsule_text.add_css_class("haze");
    capsule.append(&capsule_spin);
    capsule.append(&capsule_text);
    capsule.set_visible(false);
    header.pack_end(&capsule);
    let own = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let rows = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let end = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.append(&own);
    column.append(&rows);
    column.append(&end);
    let home = Rc::new(Home {
        ctx: Rc::downgrade(ctx),
        own,
        own_slots: RefCell::default(),
        rows,
        end,
        end_signature: RefCell::default(),
        slots: RefCell::default(),
        told: Cell::new(false),
        drawn: Cell::new(false),
        capsule,
        capsule_spin,
        activity: RefCell::default(),
        spots: RefCell::default(),
    });
    // A narrow window puts the spotlight's cover over its words, as on phones.
    ctx.on_size({
        let home = Rc::downgrade(&home);
        move |size| {
            if let Some(home) = home.upgrade() {
                home.spots.borrow_mut().retain(|(card, buttons)| {
                    let Some(card) = card.upgrade() else { return false };
                    lay_spotlight(
                        &card,
                        buttons.as_ref().and_then(|b| b.upgrade()).as_ref(),
                        size,
                    );
                    true
                });
            }
        }
    });
    home.own_slots.replace(
        [
            Landing::new({
                let home = Rc::downgrade(&home);
                move || {
                    let card = spotlight_placeholder();
                    if let Some(home) = home.upgrade() {
                        home.keep_spot(&card, None);
                    }
                    card.upcast()
                }
            }),
            Landing::new(discovery::row_placeholders),
            Landing::new(discovery::row_placeholders),
            Landing::new(discovery::row_placeholders),
        ]
        .into_iter()
        .enumerate()
        .map(|(at, landing)| home.own_slot(at, landing))
        .collect(),
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
        let (ctx, states) = (Rc::downgrade(ctx), states.clone());
        let page = page.downgrade();
        let home = Rc::clone(&home);
        move |event| {
            let Some(ctx) = ctx.upgrade() else { return };
            match event {
                Event::Reading(ReadingEvent::Home(view)) => {
                    if view.failed && super::is_blank(view) {
                        home.loading_capsule(view);
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
    /// Lay out a spotlight card, or its placeholder, for the window now and as it changes.
    fn keep_spot(&self, card: &gtk::Box, buttons: Option<&gtk::Box>) {
        let size = self.ctx.upgrade().map_or(Size::Wide, |ctx| ctx.size());
        lay_spotlight(card, buttons, size);
        let mut spots = self.spots.borrow_mut();
        spots.retain(|(card, _)| card.upgrade().is_some());
        spots.push((card.downgrade(), buttons.map(|b| b.downgrade())));
    }

    fn draw(self: &Rc<Self>, ctx: &Rc<Ctx>, view: &ReadingHomeView) {
        let demo = ctx.hub.known.borrow().demo.only_books;
        self.draw_own(ctx, view, demo);
        self.draw_rows(ctx, view);
        self.draw_end(view, demo);
        self.loading_capsule(view);
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

    /// Loading suggestions after rows have waited 4 s, gone once none is on its way. Offline is
    /// the banner's to say.
    fn loading_capsule(self: &Rc<Self>, view: &ReadingHomeView) {
        let waiting =
            !view.offline && (view.rows.iter().any(|r| r.pending) || !view.pending.is_empty());
        if !waiting {
            if let Some(id) = self.activity.take() {
                id.remove();
            }
            self.capsule_spin.set_spinning(false);
            self.capsule.set_visible(false);
            return;
        }
        if self.capsule.get_visible() || self.activity.borrow().is_some() {
            return;
        }
        let home = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(ACTIVITY, move || {
            let Some(home) = home.upgrade() else { return };
            home.activity.replace(None);
            home.capsule_spin.set_spinning(true);
            home.capsule.set_visible(true);
            motion::fade_in(&home.capsule);
        });
        self.activity.replace(Some(id));
    }

    /// A slot under `own`: the spotlight at 0, else a section with its heading.
    fn own_slot(self: &Rc<Self>, at: usize, landing: Rc<Landing>) -> OwnSlot {
        let inner = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let title = match at {
            0 => String::new(),
            1 => t::home_read_continue(),
            2 => t::home_this_week(),
            _ => t::home_up_next(),
        };
        if !title.is_empty() {
            inner.append(&heading(&title).0);
        }
        inner.append(&landing.widget);
        let part = Part::new(inner, landing, {
            let home = Rc::downgrade(self);
            move || {
                if let Some(home) = home.upgrade() {
                    home.first_gap();
                }
            }
        });
        self.own.append(&part.place);
        OwnSlot { part, title, signature: RefCell::default() }
    }

    /// The spotlight and the person's own sections, each in its own slot and drawn again only
    /// when it changed: a new pick crossfades in place, a section that arrives fades in, one
    /// that empties fades and closes its gap.
    fn draw_own(&self, ctx: &Rc<Ctx>, view: &ReadingHomeView, demo: bool) {
        let slots = self.own_slots.borrow();
        let Some((spot_slot, sections)) = slots.split_first() else { return };

        let signature = format!("{demo}{:?}{:?}", view.spotlight, view.spot); // i18n-skip
        let card: Option<gtk::Widget> = if demo {
            let top = gtk::Box::new(gtk::Orientation::Vertical, DEMO_LINE);
            if let Some(card) = super::demo::spotlight(ctx) {
                top.append(&card);
            }
            top.append(&demo_line());
            Some(top.upcast())
        } else if let Some(spot) = &view.spot
            && let Some(tile) = &view.spotlight
        {
            let art = cover(&ctx.hub, tile.cover.as_deref(), &tile.title, 160);
            let (card, buttons) = spotlight(art.upcast_ref(), &tile.work, &tile.title, spot);
            self.keep_spot(&card, Some(&buttons));
            Some(card.upcast())
        } else {
            None
        };
        match card {
            Some(card) => {
                if *spot_slot.signature.borrow() != signature || !spot_slot.part.is_shown() {
                    spot_slot.signature.replace(signature);
                    spot_slot.part.landing.land(&card);
                    spot_slot.part.reveal();
                }
            }
            None => {
                spot_slot.signature.replace(String::new());
                // On a cold load the popular pick comes with the Trending rows: its place is
                // held at final size from 600 ms so it never pushes the rows down.
                let coming = view.pending.contains(&ReadHomePart::Yours)
                    || view.rows.iter().any(|r| r.pending && r.kind == ReadRowKind::Trending);
                if coming && !view.offline {
                    spot_slot.part.landing.wait(false);
                } else {
                    spot_slot.part.leave(|| {});
                }
            }
        }

        for (slot, tiles) in
            sections.iter().zip([&view.continue_reading, &view.this_week, &view.up_next])
        {
            if tiles.is_empty() {
                slot.signature.replace(String::new());
                slot.part.leave(|| {});
                continue;
            }
            let signature = format!("{tiles:?}");
            if *slot.signature.borrow() == signature && slot.part.is_shown() {
                continue;
            }
            slot.signature.replace(signature);
            let posters = posters(ctx, tiles, Caption::Facts, false);
            slot.part.landing.land(&discovery::strip(&posters, &slot.title));
            slot.part.reveal();
        }
        drop(slots);
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
            let place = slot.part.place.clone();
            if place.is_closing() {
                continue;
            }
            if place.get_visible() {
                let rows = self.rows.clone();
                slot.part.leave(move || {
                    if place.parent().is_some() {
                        rows.remove(&place);
                    }
                });
            } else {
                self.rows.remove(&place);
            }
        }
        let mut previous: Option<gtk::Widget> = None;
        for slot in &live {
            let place = &slot.part.place;
            if place.parent().as_ref() == Some(self.rows.upcast_ref()) {
                self.rows.reorder_child_after(place, previous.as_ref());
            } else {
                self.rows.insert_child_after(place, previous.as_ref());
            }
            previous = Some(place.clone().upcast());
        }
        self.slots.replace(live);
        self.first_gap();
    }

    fn new_slot(self: &Rc<Self>, ctx: &Rc<Ctx>, row: &ReadHomeRow) -> Rc<RowSlot> {
        let inner = gtk::Box::new(gtk::Orientation::Vertical, 0);
        inner.set_margin_top(BETWEEN);
        let (head, title, side, all) = heading("");
        let query: Rc<RefCell<Option<ViewAll>>> = Rc::default();
        all.connect_clicked({
            let (ctx, query) = (Rc::downgrade(ctx), Rc::clone(&query));
            move |all| {
                // A local, so the borrow ends before view_all runs the window's action.
                let asked = query.borrow().clone();
                if let (Some(ctx), Some((query, line))) = (ctx.upgrade(), asked) {
                    view_all(&ctx, query, line, all);
                }
            }
        });
        inner.append(&head);
        let landing = Landing::new(discovery::row_placeholders);
        inner.append(&landing.widget);
        let part = Part::new(inner, landing, {
            let home = Rc::downgrade(self);
            move || {
                if let Some(home) = home.upgrade() {
                    home.first_gap();
                }
            }
        });
        Rc::new(RowSlot {
            id: row.id.clone(),
            signature: RefCell::default(),
            part,
            title,
            side,
            all,
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
        slot.query.replace(
            row.view_all.clone().map(|q| (q, row.view_all_title.as_ref().map(say::msg))),
        );
        slot.all.set_visible(row.view_all.is_some() && row.kind != ReadRowKind::Because);
        slot.all.update_property(&[gtk::accessible::Property::Label(
            &t::read_linux_view_all_row(&title),
        )]);
        if row.pending {
            slot.title.update_property(&[gtk::accessible::Property::Label(
                &t::read_linux_row_loading(&title),
            )]);
            slot.part.landing.wait(true);
            return;
        }
        slot.title.update_property(&[gtk::accessible::Property::Label(&title)]);
        let signature = format!("{:?}", row.tiles);
        if *slot.signature.borrow() == signature && slot.part.landing.is_visible() {
            return;
        }
        slot.signature.replace(signature);
        // Two facts under each title, chosen by what the row is about.
        let strip =
            discovery::strip(&posters(ctx, &row.tiles, Caption::row(row.kind), true), &title);
        // One that lands whole with nothing shown before fades in as it shows.
        slot.part.landing.land(&strip);
    }

    /// The first thing on the page starts at the top: a row with nothing above it takes no gap.
    fn first_gap(&self) {
        let mut above = false;
        for slot in self.own_slots.borrow().iter() {
            slot.part.inner.set_margin_top(if above { BETWEEN } else { 0 });
            above |= slot.part.place.get_visible();
        }
        for slot in self.slots.borrow().iter() {
            slot.part.inner.set_margin_top(if above { BETWEEN } else { 0 });
            above |= slot.part.place.get_visible();
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

/// A row's View all: its query and the line over its results.
type ViewAll = (ReadSearchQuery, Option<String>);

/// View all: Search on the Reading side, with the row's exact filters and order. Back returns
/// focus to `from`.
fn view_all(ctx: &Rc<Ctx>, query: ReadSearchQuery, line: Option<String>, from: &gtk::Button) {
    ctx.read_view_all_from.replace(Some(from.upcast_ref::<gtk::Widget>().downgrade()));
    let open = ctx.read_view_all.borrow().clone();
    if let Some(open) = open {
        open(query, line);
    }
    let _ = WidgetExt::activate_action(&ctx.window, "win.read-view-all", None);
}

/// Posters for `tiles`; `plan` offers Add to Planning on each, for discovery rows only.
fn posters(
    ctx: &Rc<Ctx>,
    tiles: &[WorkTile],
    caption: Caption,
    plan: bool,
) -> Vec<gtk::Button> {
    tiles.iter().map(|tile| discovery::poster(&ctx.hub, tile, caption, None, plan)).collect()
}

/// The spotlight's place while the popular pick is on its way: the cover's plate and bars for
/// its words, at the spotlight's size.
fn spotlight_placeholder() -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Horizontal, 20);
    card.add_css_class("read-spotlight");
    card.set_accessible_role(gtk::AccessibleRole::Presentation);
    card.append(&crate::states::plate(160, 240, "read-poster-plate"));
    let text = gtk::Box::new(gtk::Orientation::Vertical, 10);
    text.set_valign(gtk::Align::Center);
    text.set_hexpand(true);
    // No bar wider than the narrowest window's column, where the words go under the plate.
    for (width, height) in [(120, 12), (280, 28), (180, 12), (280, 12), (260, 12), (240, 12)] {
        let bar = crate::states::plate(width, height, "bar");
        bar.set_halign(gtk::Align::Start);
        text.append(&bar);
    }
    card.append(&text);
    card
}

/// The cover beside the words, or over them in a narrow window, with the main action full
/// width and Details under it, as the spotlight stacks on phones.
fn lay_spotlight(card: &gtk::Box, buttons: Option<&gtk::Box>, size: Size) {
    let narrow = size == Size::Narrow;
    card.set_orientation(if narrow {
        gtk::Orientation::Vertical
    } else {
        gtk::Orientation::Horizontal
    });
    card.set_spacing(if narrow { 14 } else { 20 });
    if let Some(buttons) = buttons {
        buttons.set_orientation(if narrow {
            gtk::Orientation::Vertical
        } else {
            gtk::Orientation::Horizontal
        });
    }
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
    label.update_property(&[gtk::accessible::Property::Level(2)]);
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

/// The spotlight's card beside `art`, its cover, and the row of its actions.
fn spotlight(
    art: &gtk::Widget,
    work: &WorkId,
    title: &str,
    spot: &kaeri_model::reading::ReadSpotlight,
) -> (gtk::Box, gtk::Box) {
    let card = gtk::Box::new(gtk::Orientation::Horizontal, 20);
    card.add_css_class("read-spotlight");
    card.append(art);
    let text = gtk::Box::new(gtk::Orientation::Vertical, 6);
    text.set_valign(gtk::Align::Center);
    text.set_hexpand(true);
    let label = widgets::haze(&say::msg(&spot.label), "caption-heading");
    label.set_xalign(0.0);
    text.append(&label);
    text.append(&widgets::show_title(title, "home-spot-title"));
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
    // One amber action: read or go on, or Add to Planning for a series no source was checked
    // for yet. Details beside it, or On your Planning list once it is planned.
    if let Some((chapter, label)) = spot.action.as_ref().and_then(action_chapter) {
        buttons.append(&read_button(&label, work, &chapter));
    } else if spot.action.is_none() && !spot.planned {
        let plan = gtk::Button::with_label(&t::library_add_planning());
        plan.add_css_class("pill");
        plan.add_css_class("suggested-action");
        plan.set_action_target_value(Some(&work.0.to_variant()));
        plan.set_action_name(Some("win.read-plan"));
        buttons.append(&plan);
    }
    let open = if spot.planned {
        let words = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let check = gtk::Image::from_icon_name("object-select-symbolic");
        check.set_accessible_role(gtk::AccessibleRole::Presentation);
        words.append(&check);
        words.append(&gtk::Label::new(Some(&t::reading_on_planning())));
        let button = gtk::Button::new();
        button.set_child(Some(&words));
        button.update_property(&[gtk::accessible::Property::Label(&t::reading_on_planning())]);
        button
    } else {
        gtk::Button::with_label(&t::home_details())
    };
    open.add_css_class("pill");
    open.set_action_target_value(Some(&work.0.to_variant()));
    open.set_action_name(Some("win.open-series"));
    buttons.append(&open);
    text.append(&buttons);
    card.append(&text);
    (card, buttons)
}

#[cfg(test)]
mod tests {
    use adw::prelude::*;
    use kaeri_core::books::{ChapterId, WorkId};
    use kaeri_model::{
        Msg, ReadingMsg,
        reading::{ReadSpotlight, ReadSpotlightCase, SeriesAction},
    };

    use super::{Size, lay_spotlight, spotlight, spotlight_placeholder};
    use crate::gtk_test::{pump, run};

    /// The spotlight and its placeholder in the smallest window, 350 px inside, whose column
    /// leaves 302 px: the cover goes over the words and the actions stack, at twice the text
    /// size too.
    #[test]
    fn the_spotlight_fits_a_narrow_window_at_any_text_size() {
        run(|| {
            let min_width =
                |widget: &gtk::Widget| widget.measure(gtk::Orientation::Horizontal, -1).0;
            let spot = ReadSpotlight {
                case: ReadSpotlightCase::Continuing,
                label: Msg::Reading(ReadingMsg::ReadingSpotlightContinue),
                facts: Vec::new(),
                synopsis: Some("A ferry crosses a lantern-lit river every night. ".repeat(4)),
                line: None,
                action: Some(SeriesAction::Continue {
                    chapter: ChapterId("13".into()),
                    label: "13".into(),
                    page: 10,
                }),
                planned: false,
                banner: None,
            };
            let art = gtk::Box::new(gtk::Orientation::Vertical, 0);
            art.set_size_request(160, 240);
            let work = WorkId("a:1".into());
            let (card, buttons) =
                spotlight(art.upcast_ref(), &work, "The Lantern Ferry", &spot);
            let placeholder = spotlight_placeholder();
            let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
            column.append(&card);
            column.append(&placeholder);
            let window = gtk::Window::builder().default_width(1280).default_height(800).build();
            window.set_child(Some(&column));
            window.present();

            lay_spotlight(&card, Some(&buttons), Size::Wide);
            pump(100);
            assert_eq!(card.orientation(), gtk::Orientation::Horizontal);
            assert_eq!(buttons.orientation(), gtk::Orientation::Horizontal);
            lay_spotlight(&card, Some(&buttons), Size::Narrow);
            lay_spotlight(&placeholder, None, Size::Narrow);
            pump(100);
            let normal = (min_width(card.upcast_ref()), min_width(placeholder.upcast_ref()));
            let settings = gtk::Settings::default().expect("a display");
            let dpi = settings.gtk_xft_dpi();
            settings.set_gtk_xft_dpi(2 * 96 * 1024);
            pump(150);
            let large = (min_width(card.upcast_ref()), min_width(placeholder.upcast_ref()));
            settings.set_gtk_xft_dpi(dpi);
            window.destroy();

            assert_eq!(card.orientation(), gtk::Orientation::Vertical);
            assert_eq!(buttons.orientation(), gtk::Orientation::Vertical);
            assert!(normal.0 <= 302, "the spotlight needs {} px at the normal size", normal.0);
            assert!(
                normal.1 <= 302,
                "its placeholder needs {} px at the normal size",
                normal.1
            );
            assert!(large.0 <= 302, "the spotlight needs {} px at twice the size", large.0);
            assert!(large.1 <= 302, "its placeholder needs {} px at twice the size", large.1);
        });
    }
}
