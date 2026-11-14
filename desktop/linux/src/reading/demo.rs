//! The reading demo in reading's places: Try it now in first run, Home's spotlight while the demo
//! is the only thing to read, the demo's series page with its credit, the reader's credit and
//! its end, its row in Search, and About the demo. Titles, makers, licences and lines come from
//! the engine's demo manifest, never from here.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    reading::{SeriesView, WorkTile},
    sources::{DemoWorkInfo, ReadingDemoInfo, reading_demo_info},
};

use super::{cover, read_target};
use crate::{pages::Dest, widgets, window::Ctx};

/// The demo's works and hosts.
pub fn info() -> ReadingDemoInfo {
    reading_demo_info()
}

/// The demo's comic, read first.
pub fn comic() -> Option<DemoWorkInfo> {
    info().works.into_iter().find(|w| !w.novel)
}

/// Open the reader on the demo's comic.
pub fn read(widget: &impl IsA<gtk::Widget>) {
    if let Some(comic) = comic() {
        let _ = widget.activate_action("win.read", Some(&read_target(&comic.work, None)));
    }
}

/// Go to Your sources, where Add is.
pub fn add_a_source(widget: &impl IsA<gtk::Widget>) {
    let _ = widget.activate_action("win.go", Some(&Dest::ReadSources.name().to_variant()));
}

fn open_link(ctx: &Rc<Ctx>, url: &str) {
    crate::portal::open_uri(&ctx.window, url, || {});
}

/// A link to `url` that opens through the portal.
fn link(ctx: &Rc<Ctx>, label: &str, url: &str) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.add_css_class("flat");
    button.set_valign(gtk::Align::Center);
    button.set_tooltip_text(Some(url));
    let (ctx, url) = (Rc::downgrade(ctx), url.to_owned());
    button.connect_clicked(move |_| {
        if let Some(ctx) = ctx.upgrade() {
            open_link(&ctx, &url);
        }
    });
    button
}

/// A work's credit as its licence asks for it, in the person's language.
pub fn credit(work: &DemoWorkInfo) -> String {
    let licence = work.licence.clone().unwrap_or_default();
    if work.novel {
        t::demo_credit_story(&work.series, &work.author)
    } else {
        t::demo_credit_book(&work.series, &work.author, &licence)
    }
}

/// The credit as one wrapping line with Licence linked inline, the way the board sets it.
fn credit_label(ctx: &Rc<Ctx>, demo: &DemoWorkInfo) -> gtk::Label {
    let mut markup = glib::markup_escape_text(&credit(demo)).to_string();
    if let Some(url) = &demo.licence_url {
        markup.push_str(&format!(
            " <a href=\"{}\">{}</a>", // i18n-skip
            glib::markup_escape_text(url),
            glib::markup_escape_text(&t::sources_demo_link_licence())
        ));
    }
    let label = gtk::Label::new(None);
    label.set_markup(&markup);
    label.add_css_class("haze");
    label.add_css_class("demo-credit-line");
    label.set_wrap(true);
    label.set_xalign(0.0);
    label.set_max_width_chars(52);
    label.connect_activate_link({
        let ctx = Rc::downgrade(ctx);
        move |_, url| {
            if let Some(ctx) = ctx.upgrade() {
                open_link(&ctx, url);
            }
            glib::Propagation::Stop
        }
    });
    label
}

// ---------------------------------------------------------------------------------------------
// First run

/// Try it now: the covers, what the demo is, and Read the demo (and Play the demo after Both).
pub fn try_card(ctx: &Rc<Ctx>, both: bool, done: impl Fn() + Clone + 'static) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    card.add_css_class("card");
    card.add_css_class("demo-try");
    let covers = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    covers.set_valign(gtk::Align::Center);
    let works = info().works;
    if both {
        let art = kaeri_model::sources::demo_info().art;
        covers.append(&cover(&ctx.hub, art.as_deref(), &t::sources_demo_name(), 56));
    }
    for work in works.iter().filter(|w| !both || !w.novel) {
        covers.append(&cover(&ctx.hub, work.cover.as_deref(), &work.series, 56));
    }
    card.append(&covers);
    let words = gtk::Box::new(gtk::Orientation::Vertical, 4);
    words.set_valign(gtk::Align::Center);
    words.set_hexpand(true);
    let heading = gtk::Label::new(Some(&t::onboarding_demo_heading()));
    heading.add_css_class("heading");
    heading.set_xalign(0.0);
    words.append(&heading);
    words.append(&widgets::haze(
        &if both {
            t::onboarding_demo_both_detail()
        } else {
            t::onboarding_demo_books_detail()
        },
        "",
    ));
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    if both {
        let play = gtk::Button::with_label(&t::onboarding_play_demo());
        play.add_css_class("flat");
        play.connect_clicked({
            let (ctx, done) = (Rc::downgrade(ctx), done.clone());
            move |_| {
                done();
                if let Some(ctx) = ctx.upgrade() {
                    ctx.hub.send(Command::PlayDemo);
                }
            }
        });
        buttons.append(&play);
    }
    let read_it = gtk::Button::with_label(&t::onboarding_read_demo());
    read_it.add_css_class("flat");
    read_it.connect_clicked({
        let window = ctx.window.clone();
        move |_| {
            done();
            read(&window);
        }
    });
    buttons.append(&read_it);
    words.append(&buttons);
    card.append(&words);
    ctx.hub
        .send(Command::BookSources(kaeri_model::book_sources::BookSourceCommand::PrefetchDemo));
    card
}

// ---------------------------------------------------------------------------------------------
// Home

/// Home's spotlight while the demo is the only thing to read: the comic's art across, Demo, free
/// to share, its title, its line and Read.
pub fn spotlight(ctx: &Rc<Ctx>) -> Option<gtk::Widget> {
    let comic = comic()?;
    let overlay = gtk::Overlay::new();
    overlay.add_css_class("demo-spotlight");
    overlay.set_overflow(gtk::Overflow::Hidden);
    // A band, as the board draws it: the band sets the height, and the art fills it as a layer
    // over it, so the cover's own size never stretches the card.
    let band = gtk::Box::new(gtk::Orientation::Vertical, 0);
    band.set_size_request(-1, 300);
    band.set_hexpand(true);
    overlay.set_child(Some(&band));
    overlay.set_valign(gtk::Align::Start);
    let (art, _) = widgets::art(&ctx.hub, comic.cover.as_deref(), None, "demo-spotlight-art");
    overlay.add_overlay(&art);
    // The cover is a whole page, white margins and all. The art layer runs a little past both
    // sides so the band shows only the drawing; the card's rounded clip trims the rest.
    let layer = art.clone().upcast::<gtk::Widget>();
    overlay.connect_get_child_position(move |overlay, widget| {
        if widget != &layer {
            return None;
        }
        let (width, height) = (overlay.width(), overlay.height());
        let bleed = width / 12;
        Some(gtk::gdk::Rectangle::new(-bleed, 0, width + 2 * bleed, height))
    });
    let shade = gtk::Box::new(gtk::Orientation::Vertical, 0);
    shade.add_css_class("demo-spotlight-shade");
    overlay.add_overlay(&shade);
    let words = gtk::Box::new(gtk::Orientation::Vertical, 4);
    words.set_valign(gtk::Align::End);
    words.set_halign(gtk::Align::Start);
    words.set_margin_start(28);
    words.set_margin_end(28);
    words.set_margin_bottom(26);
    let label = gtk::Label::new(Some(&t::home_spotlight_demo().to_uppercase()));
    label.add_css_class("caption-heading");
    label.add_css_class("demo-spotlight-label");
    label.set_xalign(0.0);
    words.append(&label);
    let title = widgets::show_title(&comic.series, "page");
    title.set_accessible_role(gtk::AccessibleRole::Heading);
    words.append(&title);
    if !comic.synopsis.is_empty() {
        let line = gtk::Label::new(Some(&comic.synopsis));
        line.set_xalign(0.0);
        line.set_wrap(true);
        line.set_lines(2);
        line.set_ellipsize(gtk::pango::EllipsizeMode::End);
        line.set_margin_bottom(10);
        words.append(&line);
    }
    let read_it = gtk::Button::with_label(&t::home_spotlight_read());
    read_it.add_css_class("pill");
    read_it.add_css_class("suggested-action");
    read_it.set_halign(gtk::Align::Start);
    read_it.set_action_name(Some("win.read"));
    read_it.set_action_target_value(Some(&read_target(&comic.work, None)));
    words.append(&read_it);
    overlay.add_overlay(&words);
    super::opens_series(&overlay, &comic.work);
    Some(overlay.upcast())
}

// ---------------------------------------------------------------------------------------------
// The series page

/// A demo work's page: the cover beside its facts, title and credit with the licence linked,
/// Read chapter 1, its one chapter, and the demo's other work. No lists, tracking or downloads.
pub fn series_body(ctx: &Rc<Ctx>, series: &SeriesView, demo: &DemoWorkInfo) -> adw::Clamp {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 18);
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 20);
    let art = cover(&ctx.hub, demo.cover.as_deref(), &demo.series, 180);
    top.append(&art);
    let words = gtk::Box::new(gtk::Orientation::Vertical, 6);
    words.set_valign(gtk::Align::Center);
    words.set_hexpand(true);
    let facts = if demo.novel {
        t::demo_story_row(&demo.author, demo.licence.as_deref().unwrap_or_default())
    } else {
        t::demo_series_facts(i64::from(demo.pages))
    };
    let facts = widgets::haze(&facts.to_uppercase(), "caption-heading");
    words.append(&facts);
    words.append(&widgets::show_title(&demo.series, "page"));
    words.append(&credit_label(ctx, demo));
    let read_it = gtk::Button::with_label(&t::demo_read_chapter("1"));
    read_it.add_css_class("pill");
    read_it.add_css_class("suggested-action");
    read_it.set_halign(gtk::Align::Start);
    read_it.set_margin_top(10);
    read_it.set_action_name(Some("win.read"));
    read_it.set_action_target_value(Some(&read_target(&series.work, None)));
    words.append(&read_it);
    top.append(&words);
    column.append(&top);

    column.append(&widgets::heading(&t::demo_chapters()));
    let chapters = widgets::boxed_list();
    let row = adw::ActionRow::builder()
        .title(glib::markup_escape_text(&t::reading_chapter_titled("1", &demo.chapter)))
        .activatable(true)
        .build();
    if demo.pages > 0 {
        row.set_subtitle(&t::demo_pages(i64::from(demo.pages)));
    }
    row.add_suffix(&widgets::chevron());
    row.set_action_name(Some("win.read"));
    row.set_action_target_value(Some(&read_target(&series.work, None)));
    chapters.append(&row);
    column.append(&chapters);

    if let (Some(other), Some(work)) =
        (info().works.into_iter().find(|w| w.id != demo.id), series.demo_other.as_ref())
    {
        column.append(&widgets::heading(&t::demo_also()));
        let also = widgets::boxed_list();
        let line = if other.novel {
            t::demo_story_row(&other.author, other.licence.as_deref().unwrap_or_default())
        } else {
            t::demo_series_facts(i64::from(other.pages))
        };
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&other.series))
            .subtitle(glib::markup_escape_text(&line))
            .activatable(true)
            .build();
        row.add_prefix(&cover(&ctx.hub, other.cover.as_deref(), &other.series, 36));
        row.add_suffix(&widgets::chevron());
        row.set_action_name(Some("win.open-series"));
        row.set_action_target_value(Some(&work.0.to_variant()));
        also.append(&row);
        column.append(&also);
    }
    adw::Clamp::builder().maximum_size(640).tightening_threshold(640).child(&column).build()
}

// ---------------------------------------------------------------------------------------------
// The reader

/// The credit capsule in the reader's top bar on page 1: title, maker, licence.
pub fn credit_capsule(demo: &DemoWorkInfo) -> gtk::Box {
    let capsule = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    capsule.add_css_class("demo-credit");
    capsule.set_valign(gtk::Align::Center);
    let icon = gtk::Image::from_icon_name("help-about-symbolic");
    icon.set_pixel_size(12);
    icon.set_accessible_role(gtk::AccessibleRole::Presentation);
    capsule.append(&icon);
    let words = gtk::Label::new(Some(&t::reader_demo_credit(
        &demo.series,
        &demo.author,
        demo.licence.as_deref().unwrap_or_default(),
    )));
    words.add_css_class("caption");
    words.set_ellipsize(gtk::pango::EllipsizeMode::End);
    capsule.append(&words);
    capsule
}

/// The card after the demo's last page: the cover, that the demo ends here, Add a source and
/// Back to page 1.
pub fn end_card(
    ctx: &Rc<Ctx>,
    demo: &DemoWorkInfo,
    close: impl Fn() + 'static,
    again: impl Fn() + 'static,
) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
    card.set_halign(gtk::Align::Center);
    card.set_valign(gtk::Align::Center);
    card.set_size_request(420, -1);
    card.add_css_class("demo-end");
    let art = cover(&ctx.hub, demo.cover.as_deref(), &demo.series, 72);
    art.set_halign(gtk::Align::Center);
    card.append(&art);
    let title = gtk::Label::new(Some(&t::reader_demo_end()));
    title.add_css_class("title-4");
    title.set_margin_top(10);
    card.append(&title);
    let line = gtk::Label::new(Some(&t::reader_demo_end_line()));
    line.add_css_class("haze");
    line.set_wrap(true);
    line.set_justify(gtk::Justification::Center);
    line.set_max_width_chars(44);
    card.append(&line);
    let add = gtk::Button::with_label(&t::reader_demo_add_source());
    add.add_css_class("suggested-action");
    add.set_margin_top(12);
    add.connect_clicked(move |button| {
        let window = button.root().and_downcast::<gtk::Window>();
        close();
        if let Some(window) = window {
            add_a_source(&window);
        }
    });
    card.append(&add);
    let back = gtk::Button::with_label(&t::reader_demo_again());
    back.connect_clicked(move |_| again());
    card.append(&back);
    card
}

// ---------------------------------------------------------------------------------------------
// Search

/// Search while only the demo is on: its works as rows, Demo · Reads now, and the line that
/// other titles come with a source.
pub fn search_results(ctx: &Rc<Ctx>, tiles: &[WorkTile]) -> adw::Clamp {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 10);
    column.set_margin_top(12);
    column.set_margin_start(12);
    column.set_margin_end(12);
    if !tiles.is_empty() {
        let list = widgets::boxed_list();
        for tile in tiles {
            let row = adw::ActionRow::builder()
                .title(glib::markup_escape_text(&tile.title))
                .activatable(true)
                .build();
            row.add_css_class("demo-result");
            if tile.demo {
                row.set_subtitle(&t::search_demo_reads());
            }
            row.add_prefix(&cover(&ctx.hub, tile.cover.as_deref(), &tile.title, 36));
            row.set_action_name(Some("win.open-series"));
            row.set_action_target_value(Some(&tile.work.0.to_variant()));
            list.append(&row);
        }
        column.append(&list);
    }
    let note = widgets::haze(&t::search_demo_only_books(), "");
    note.set_xalign(0.0);
    note.set_wrap(true);
    column.append(&note);
    adw::Clamp::builder().maximum_size(640).tightening_threshold(640).child(&column).build()
}

// ---------------------------------------------------------------------------------------------
// About the demo

/// About the demo, from Your sources: what it is, its credits with their licences, the only
/// hosts it reaches, that it signs in nowhere, and Remove demo.
pub fn about_page(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 18);
    let what = gtk::Label::new(Some(&t::sources_demo_books_what()));
    what.set_wrap(true);
    what.set_xalign(0.0);
    column.append(&what);
    column.append(&widgets::heading(&t::demo_credits()));
    let credits = widgets::boxed_list();
    for work in &info().works {
        // The story's cover credit goes under its own credit, as its licence asks.
        let mut line = credit(work);
        if let Some(art) = &work.cover_credit {
            line.push('\n');
            line.push_str(&t::demo_credit_cover(
                &art.title,
                &art.artist,
                &art.year.to_string(),
            ));
        }
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&work.series))
            .subtitle(glib::markup_escape_text(&line))
            .subtitle_lines(0)
            .build();
        row.add_prefix(&cover(&ctx.hub, work.cover.as_deref(), &work.series, 36));
        if let Some(url) = &work.licence_url {
            row.add_suffix(&link(ctx, &t::sources_demo_link_licence(), url));
        }
        credits.append(&row);
    }
    column.append(&credits);
    let facts = widgets::boxed_list();
    for line in [t::sources_demo_hosts(), t::sources_demo_books_private()] {
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&line))
            .title_lines(0)
            .build();
        facts.append(&row);
    }
    column.append(&facts);
    let remove_list = widgets::boxed_list();
    let remove = adw::ActionRow::builder()
        .title(glib::markup_escape_text(&t::sources_demo_remove()))
        .activatable(true)
        .build();
    remove.add_prefix(&super::came_in::tile("user-trash-symbolic"));
    remove.connect_activated({
        let ctx = Rc::downgrade(ctx);
        move |row| {
            if let Some(ctx) = ctx.upgrade() {
                ctx.hub.send(Command::RemoveDemo);
                if let Some(nav) = row
                    .ancestor(adw::NavigationView::static_type())
                    .and_downcast::<adw::NavigationView>()
                {
                    nav.pop();
                }
            }
        }
    });
    remove_list.append(&remove);
    column.append(&remove_list);
    let header = adw::HeaderBar::new();
    widgets::read_content_page(
        &t::sources_demo_about(),
        &header,
        &widgets::column_page(&column, 640),
    )
}
