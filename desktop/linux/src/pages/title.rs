//! A title's page: the banner, then the cover beside the show title, the metadata line, the one
//! amber action and the status and More menus; under them the season picker and the episodes,
//! and the details and Related in a side column. One column below the wide size, and on narrow
//! windows no cover beside the title, the action at full width and smaller stills.

use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    rc::{Rc, Weak},
};

use adw::prelude::*;
use gtk::{gio, glib};
use kaeri_core::ids::AnilistId;
use kaeri_i18n::t;
use kaeri_model::{
    catalog::{Entry, LibrarySegment, LinkedTitle, RelationKind},
    command::{Command, LicensedCommand},
    episodes::{self, EpisodeRange, EpisodeRow, EpisodeText},
    event::Event,
    licensed::{AvailabilityCase, TitleAvailability},
    settings::SettingKey,
    show::ShowSettingKey,
};

use crate::{
    catalog, choices, matching, motion,
    pages::licensed,
    say,
    states::{Shape, States},
    synopsis, widgets,
    window::{Ctx, Size},
};

/// The banner's tallest height: the band the head stands over.
const BAND: i32 = 328;

/// How far a watched episode's still dims.
const WATCHED: f64 = 0.55;

/// An episode row as drawn, kept so a mark changes it in place: focus stays where it was, and
/// the still dims on the quick token.
struct EpisodeLine {
    number: String,
    check: gtk::CheckButton,
    toggled: glib::SignalHandlerId,
    name: gtk::Label,
    still: gtk::Widget,
    /// The row's content, dimmed when no source lists the episode.
    root: gtk::Box,
    sub: gtk::Label,
    /// "Discussion · 214 comments", once a thread is known.
    discuss: gtk::Button,
    bar: gtk::ProgressBar,
    menu: gio::Menu,
    shown: RefCell<EpisodeRow>,
    /// The row itself, which carries its tooltip and what a screen reader says of it.
    list_row: gtk::ListBoxRow,
    /// The synopsis, and the box that keeps its room (three lines).
    synopsis: gtk::Label,
    room_box: gtk::Box,
    /// The blurred copy of the still over the sharp one, while spoiler protection holds the row.
    blur: Option<gtk::Widget>,
    /// Spoiler protection holds the row back now.
    protected: Cell<bool>,
    /// The row kept room for text when it was laid out (and does now: a reveal gives it).
    room: Cell<bool>,
    /// The height the row has now.
    height: Cell<i32>,
}

/// What the episode list was drawn from: when a table arrives that differs in none of it, the
/// rows change in place; otherwise the list is drawn again.
#[derive(PartialEq)]
struct Drawn {
    numbers: Vec<String>,
    narrow: bool,
    range: Option<usize>,
    fold: usize,
    protection_at: Option<usize>,
}

/// The one line where spoiler protection starts.
struct Protection {
    label: gtk::Label,
    button: gtk::Button,
    /// The last episode before it.
    episode: i64,
}

struct Page {
    ctx: Rc<Ctx>,
    id: AnilistId,
    navigation: glib::WeakRef<adw::NavigationPage>,
    banner: gtk::Box,
    banner_filled: Cell<bool>,
    head: gtk::Box,
    side: gtk::Box,
    seasons: gtk::Box,
    synopsis: gtk::Box,
    episodes: gtk::ListBox,
    episodes_state: Rc<States>,
    lines: RefCell<Vec<EpisodeLine>>,
    /// The page itself, for what a row's buttons call.
    me: RefCell<Weak<Page>>,
    /// The scrolled page, kept so unfolding the watched rows leaves the next one where it was.
    scroll: gtk::ScrolledWindow,
    /// Over the episode list: the snapshot a range switch crossfades from.
    veil: gtk::Overlay,
    /// The range picker, beside the heading of the list.
    range_slot: gtk::Box,
    /// Whether the table had text, expected text or none when it was first laid out: kept until
    /// the next layout (a range switch, or the page coming back), so rows never grow on screen.
    outlook: Cell<Option<EpisodeText>>,
    /// The episodes the viewer asked to see through spoiler protection, for this visit.
    revealed: RefCell<HashSet<String>>,
    /// Spoiler protection's line was asked to show every row.
    reveal_all: Cell<bool>,
    /// What the show sets for itself in spoiler protection, once the engine said.
    own_spoilers: Cell<Option<bool>>,
    /// The range showing, an index into the table's ranges; none until the page chooses.
    range: Cell<Option<usize>>,
    /// The line of folded watched episodes was opened.
    unfolded: Cell<bool>,
    /// What the list was drawn from, to tell a change in place from one that needs a new layout.
    drawn: RefCell<Option<Drawn>>,
    /// The protection line, once drawn: its text, its button and where it stands.
    protection: RefCell<Option<Protection>>,
    /// Whether the engine has answered for the episodes, so no table means none.
    listed: Cell<bool>,
    columns: gtk::Box,
    rows: Rc<RefCell<Vec<EpisodeRow>>>,
    /// The page's status action, its state the list the title is on.
    status: gio::SimpleAction,
    /// How far the dub goes, from the source a play would use: "Dub to episode 6".
    dubbed_to: RefCell<Option<String>>,
    /// The serial of the newest pick whose notice was said.
    said_match: Cell<u32>,
    /// Fix this match, enabled while there is a source to match on.
    fix_match: RefCell<Option<gio::SimpleAction>>,
    listening: RefCell<Option<u64>>,
    sized: RefCell<Size>,
    /// The page's own heading, and the header bar that shows the title once it scrolls away.
    heading: RefCell<Option<gtk::Label>>,
    header: adw::HeaderBar,
    /// Play on TV, at the header's end: the episode the primary action plays.
    tv_button: gtk::Button,
    content: gtk::Box,
}

/// The tag a title's page carries in the navigation stack: one page per title.
pub fn tag(id: AnilistId) -> String {
    format!("title-{}", id.get())
}

/// How opening a title meets the stack, bottom to top by tag.
#[derive(Debug, PartialEq, Eq)]
pub enum Reach {
    /// Not in the stack: a new page.
    Push,
    /// Already on top.
    Stay,
    /// Further down: back to it, rather than a second page of the same title.
    PopTo,
}

pub fn reach(stack: &[Option<String>], tag: &str) -> Reach {
    match stack.iter().rposition(|t| t.as_deref() == Some(tag)) {
        None => Reach::Push,
        Some(at) if at + 1 == stack.len() => Reach::Stay,
        Some(_) => Reach::PopTo,
    }
}

pub fn page(ctx: &Rc<Ctx>, id: AnilistId) -> adw::NavigationPage {
    let entry = ctx.hub.entry(id);
    // Nothing in the header until the title is known, rather than a word standing in for it.
    let title = entry.as_ref().map_or("", |e| e.title.as_str()).to_owned();

    let head = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let seasons = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let synopsis = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let episodes = gtk::ListBox::new();
    // Rows have no dividers: the still anchors each one.
    episodes.add_css_class("episode-list");
    episodes.set_selection_mode(gtk::SelectionMode::None);
    let none = widgets::haze(&t::title_no_episodes(), "");
    none.set_valign(gtk::Align::Start);
    let shape = Shape::Rows { count: 4, width: 128, height: 72 };
    let episodes_state =
        States::new(&ctx.hub, &episodes, shape, &none, &t::title_episodes_load_failed());
    episodes_state.widget.set_vhomogeneous(false);
    episodes_state.widget.set_vexpand(false);
    episodes_state.loading(Some(t::activity_episodes()));
    episodes_state.on_retry({
        let hub = Rc::downgrade(&ctx.hub);
        let state = Rc::downgrade(&episodes_state);
        move || {
            if let (Some(hub), Some(state)) = (hub.upgrade(), state.upgrade()) {
                state.loading(Some(t::activity_episodes()));
                hub.send(Command::LoadEpisodes(id));
            }
        }
    });

    let main = gtk::Box::new(gtk::Orientation::Vertical, 12);
    main.set_hexpand(true);
    main.append(&synopsis);
    main.append(&seasons);
    let heading = widgets::heading(&t::title_episodes());
    heading.set_margin_top(12);
    // The heading, and beside it the range picker of a long series.
    let range_slot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    range_slot.set_valign(gtk::Align::End);
    let heading_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    heading_row.append(&heading);
    heading_row.append(&range_slot);
    main.append(&heading_row);
    // Over the list, a range switch lays the old list's picture and fades it out.
    let veil = gtk::Overlay::new();
    veil.set_child(Some(&episodes_state.widget));
    main.append(&veil);

    let side = gtk::Box::new(gtk::Orientation::Vertical, 18);
    side.set_size_request(260, -1);
    let columns = gtk::Box::new(gtk::Orientation::Horizontal, 36);
    columns.append(&main);
    columns.append(&side);
    columns.set_margin_start(36);
    columns.set_margin_end(36);
    columns.set_margin_top(18);
    columns.set_margin_bottom(36);

    // The banner, up to 360 high, with the cover and the title over its lower part, on the
    // ground's fade: both in one cell of a grid, so the head may run past the banner.
    let banner_slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    banner_slot.set_valign(gtk::Align::Start);
    head.set_valign(gtk::Align::End);
    head.set_margin_top(120);
    head.set_margin_start(36);
    head.set_margin_end(36);
    let head_clamp = adw::Clamp::builder()
        .maximum_size(1200)
        .tightening_threshold(1200)
        .child(&head)
        .build();
    let top = gtk::Grid::new();
    top.attach(&banner_slot, 0, 0, 1, 1);
    top.attach(&head_clamp, 0, 0, 1, 1);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&top);
    content.append(
        &adw::Clamp::builder()
            .maximum_size(1200)
            .tightening_threshold(1200)
            .child(&columns)
            .build(),
    );
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&content)
        .build();

    let header = adw::HeaderBar::new();
    header.add_css_class("flat");
    // The heading says the title; the header bar says it only once the heading has gone.
    header.set_show_title(false);
    let tv_button = gtk::Button::from_icon_name("video-display-symbolic");
    tv_button.set_tooltip_text(Some(&t::tv_play_on()));
    tv_button.update_property(&[gtk::accessible::Property::Description(
        &t::title_linux_play_on_tv_help(),
    )]);
    tv_button.set_visible(false);
    header.pack_end(&tv_button);
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.add_top_bar(&widgets::offline_banner());
    view.set_extend_content_to_top_edge(true);
    view.set_content(Some(&scroll));
    let navigation =
        adw::NavigationPage::builder().title(&title).tag(tag(id)).child(&view).build();

    let page = Rc::new(Page {
        ctx: Rc::clone(ctx),
        id,
        navigation: navigation.downgrade(),
        banner: banner_slot,
        banner_filled: Cell::new(false),
        head,
        side,
        seasons,
        synopsis,
        episodes,
        episodes_state,
        lines: RefCell::default(),
        me: RefCell::default(),
        scroll: scroll.clone(),
        veil,
        range_slot,
        outlook: Cell::new(None),
        revealed: RefCell::default(),
        reveal_all: Cell::new(false),
        own_spoilers: Cell::new(None),
        range: Cell::new(None),
        unfolded: Cell::new(false),
        drawn: RefCell::default(),
        protection: RefCell::default(),
        listed: Cell::new(false),
        columns,
        rows: Rc::default(),
        listening: RefCell::default(),
        sized: RefCell::new(ctx.size()),
        status: gio::SimpleAction::new_stateful(
            "status",
            Some(glib::VariantTy::STRING),
            &String::new().to_variant(),
        ),
        dubbed_to: RefCell::default(),
        fix_match: RefCell::default(),
        said_match: Cell::new(ctx.hub.known.borrow().matches.settled().map_or(0, |s| s.serial)),
        heading: RefCell::default(),
        header,
        tv_button,
        content: content.clone(),
    });
    scroll.vadjustment().connect_value_changed({
        let weak = Rc::downgrade(&page);
        move |adjustment| {
            if let Some(page) = weak.upgrade() {
                page.title_shown(adjustment.value());
            }
        }
    });

    let weak = Rc::downgrade(&page);
    let listening = ctx.hub.listen(move |event| {
        if let Some(page) = weak.upgrade() {
            page.heard(event);
        }
    });
    page.listening.replace(Some(listening));
    ctx.on_size({
        let weak = Rc::downgrade(&page);
        move |size| {
            if let Some(page) = weak.upgrade() {
                page.resized(size);
            }
        }
    });
    page.me.replace(Rc::downgrade(&page));
    page.actions();
    ctx.hub.known.borrow_mut().visit(id);
    if let Some(entry) = &entry {
        page.fill(entry);
    }
    ctx.hub.send(Command::LoadDetail(id));
    ctx.hub.send(Command::LoadEpisodes(id));
    ctx.hub.send(Command::LoadDub(id));
    // The show's own spoiler protection, which the rows follow.
    ctx.hub.send(Command::LoadShowSettings { id });
    // Kept as long as the page it draws, and deaf once that is gone.
    navigation.connect_destroy(move |_| {
        if let Some(id) = page.listening.borrow_mut().take() {
            page.ctx.hub.forget(id);
        }
    });
    navigation
}

impl Page {
    /// The header bar's title, shown once the page's heading has scrolled under it.
    fn title_shown(&self, scrolled: f64) {
        let gone = self.heading.borrow().as_ref().and_then(|heading| {
            let bottom = gtk::graphene::Point::new(0.0, heading.height() as f32);
            heading.compute_point(&self.content, &bottom)
        });
        // The header bar stands over the banner: about 47 pixels.
        let shown = gone.is_some_and(|point| scrolled > f64::from(point.y()) - 47.0);
        if self.header.shows_title() != shown {
            self.header.set_show_title(shown);
        }
    }

    fn heard(&self, event: &Event) {
        match event {
            // Titles are named another way now: this one and its related titles again.
            Event::TitlesChanged(_) => self.ctx.hub.send(Command::LoadDetail(self.id)),
            Event::Detail(entry) if entry.id == self.id => {
                if let Some(navigation) = self.navigation.upgrade() {
                    navigation.set_title(&entry.title);
                }
                self.fill(entry);
            }
            Event::Episodes { id, rows, .. } if *id == self.id => {
                self.listed.set(true);
                let same = {
                    let old = self.rows.borrow();
                    old.len() == rows.len()
                        && old.iter().zip(rows).all(|(a, b)| a.number == b.number)
                };
                self.rows.replace(rows.clone());
                self.ask_availability();
                // The same table with more in it (text that arrived late, a mark): the rows
                // change in place, and only a change in what the list is made of draws it again.
                if same && self.drawn.borrow().is_some() {
                    self.update_episodes();
                } else {
                    self.draw_episodes();
                }
                // The table decides what More offers.
                if let Some(entry) = self.ctx.hub.entry(self.id) {
                    self.fill(&entry);
                }
            }
            // The candidates come first, then AniList's table until a match is picked.
            Event::MatchChoices { id, answers, .. } if *id == self.id => {
                // A search by hand answers the sheet, not the page.
                if matches!(answers, kaeri_model::sources::MatchQuestion::Ladder) {
                    self.listed.set(true);
                    self.episodes_state.empty();
                }
                self.redress();
            }
            // What the title is matched to now: the question answered, or a pick settled.
            Event::TitleMatch { id, .. } if *id == self.id => {
                self.redress();
                let settled = self
                    .ctx
                    .hub
                    .known
                    .borrow()
                    .matches
                    .settled()
                    .filter(|s| s.id == self.id && s.serial > self.said_match.get())
                    .cloned();
                if let Some(settled) = settled {
                    self.said_match.set(settled.serial);
                    self.ctx.toast(&t::match_picked(&settled.title, &settled.source));
                }
            }
            // The source pinned for this title is off, gone or resting, so its episodes come
            // from the others. Said once, until a source is chosen again.
            Event::PinUnavailable { id, provider_id, name } if *id == self.id => {
                let new = self.ctx.hub.known.borrow_mut().say_pin(*id, provider_id);
                if new {
                    self.ctx.toast(&t::source_pin_unavailable(name));
                }
            }
            Event::DiscussionFound { media, .. } if *media == self.id => {
                self.redress_discussions();
            }
            // What the page says about licensed services: the buttons and lines change in
            // place, and the episodes take their links.
            Event::Availability(availability) if availability.id == self.id => {
                self.draw_episodes();
                if let Some(entry) = self.ctx.hub.entry(self.id) {
                    self.fill(&entry);
                }
            }
            // The services were chosen again: ask what this title says now.
            Event::Licensed(_) if self.listed.get() => self.ask_availability(),
            Event::TitleDub { id, dub } if *id == self.id => {
                self.dubbed_to.replace(say::dub_line(dub.as_ref()));
                if let Some(entry) = self.ctx.hub.entry(self.id) {
                    self.fill(&entry);
                }
            }
            Event::Progress { id, episodes, .. } if *id == self.id => {
                {
                    let mut rows = self.rows.borrow_mut();
                    for row in rows.iter_mut() {
                        if let Some(p) = episodes.iter().find(|p| p.number == row.number) {
                            row.watched = p.watched;
                            row.completed = p.completed;
                        }
                    }
                }
                if let Some(entry) = self.ctx.hub.entry(self.id) {
                    self.fill(&entry);
                }
                self.update_episodes();
                // Which rows are ahead moved with the mark.
                self.ctx.hub.send(Command::LoadEpisodes(self.id));
            }
            Event::ShowSettings(view) if view.id == self.id => {
                let own = view
                    .rows
                    .iter()
                    .find(|row| row.key == ShowSettingKey::SpoilerProtection)
                    .map(|row| row.value == "on");
                if own != self.own_spoilers.get() {
                    self.own_spoilers.set(own);
                    if self.drawn.borrow().is_some() {
                        self.draw_episodes();
                    }
                }
            }
            _ => {}
        }
    }

    /// Ask what this title says about licensed services, once the episodes say whether it plays
    /// here.
    fn ask_availability(&self) {
        let plays_here = self.rows.borrow().iter().any(choices::playable);
        self.ctx.hub.send(Command::Licensed(LicensedCommand::LoadAvailability {
            id: self.id,
            plays_here,
        }));
    }

    /// What the page says about licensed services, as the engine last worked it out.
    fn availability(&self) -> Option<TitleAvailability> {
        self.ctx.hub.known.borrow().availability.get(&self.id.get()).cloned()
    }

    fn resized(&self, size: Size) {
        self.sized.replace(size);
        let wide = size == Size::Wide;
        self.columns.set_orientation(if wide {
            gtk::Orientation::Horizontal
        } else {
            gtk::Orientation::Vertical
        });
        self.side.set_size_request(if wide { 260 } else { -1 }, -1);
        let margin = if size == Size::Narrow { 12 } else { 36 };
        self.columns.set_margin_start(margin);
        self.columns.set_margin_end(margin);
        self.head.set_margin_start(margin);
        self.head.set_margin_end(margin);
        if let Some(entry) = self.ctx.hub.entry(self.id) {
            self.fill(&entry);
        }
        self.draw_episodes();
    }

    /// The page's own actions: the status menu and More.
    fn actions(&self) {
        let group = gio::SimpleActionGroup::new();
        let status = &self.status;
        let hub = Rc::downgrade(&self.ctx.hub);
        let id = self.id;
        status.connect_activate({
            let hub = hub.clone();
            move |action, arg| {
                let Some(hub) = hub.upgrade() else { return };
                let Some(name) = arg.and_then(String::from_variant) else { return };
                if let Some(status) =
                    LibrarySegment::ALL.into_iter().find(|s| s.label() == name)
                {
                    action.set_state(&name.to_variant());
                    hub.send(Command::SetStatus { id, status });
                    hub.send(Command::LoadDetail(id));
                }
            }
        });
        group.add_action(status);
        let trailer = gio::SimpleAction::new("trailer", None);
        trailer.connect_activate({
            let ctx = Rc::downgrade(&self.ctx);
            move |_, _| {
                let Some(ctx) = ctx.upgrade() else { return };
                let Some(entry) = ctx.hub.entry(id) else { return };
                crate::trailer::play_window(&ctx, &entry);
            }
        });
        group.add_action(&trailer);
        let download = gio::SimpleAction::new("download-unwatched", None);
        download.connect_activate({
            let hub = hub.clone();
            let rows = Rc::clone(&self.rows);
            move |_, _| {
                let Some(hub) = hub.upgrade() else { return };
                let episodes = unwatched(&rows.borrow());
                if !episodes.is_empty() {
                    hub.send(Command::DownloadMany { id, episodes });
                }
            }
        });
        group.add_action(&download);
        let previous = gio::SimpleAction::new("mark-previous", None);
        previous.connect_activate({
            let hub = hub.clone();
            move |_, _| {
                let Some(hub) = hub.upgrade() else { return };
                let Some(entry) = hub.entry(id) else { return };
                let next = entry.progress.map_or(1, |p| p.next);
                let episodes: Vec<String> = (1..next).map(|n| n.to_string()).collect();
                if !episodes.is_empty() {
                    hub.send(Command::SetWatched { id, episodes, watched: true });
                }
            }
        });
        group.add_action(&previous);
        let show_settings = gio::SimpleAction::new("show-settings", None);
        show_settings.connect_activate({
            let ctx = Rc::downgrade(&self.ctx);
            move |_, _| {
                let Some(ctx) = ctx.upgrade() else { return };
                let title = ctx.hub.entry(id).map(|e| e.title).unwrap_or_default();
                crate::pages::show_settings::open(&ctx, id, &title);
            }
        });
        group.add_action(&show_settings);
        let source = gio::SimpleAction::new("source-picker", None);
        source.connect_activate({
            let ctx = Rc::downgrade(&self.ctx);
            move |_, _| {
                let Some(ctx) = ctx.upgrade() else { return };
                let _ = crate::pages::source_picker::open(&ctx, &ctx.window, id);
            }
        });
        group.add_action(&source);
        // Choose the match is the page's own question; Fix this match searches for the title.
        for (name, from_menu) in [("choose-match", false), ("fix-match", true)] {
            let action = gio::SimpleAction::new(name, None);
            action.connect_activate({
                let ctx = Rc::downgrade(&self.ctx);
                move |_, _| {
                    let Some(ctx) = ctx.upgrade() else { return };
                    let from_menu = from_menu && !ctx.hub.known.borrow().matches.asks(id);
                    let _ = crate::pages::match_picker::open(
                        &ctx,
                        &ctx.window,
                        id,
                        from_menu,
                        None,
                    );
                }
            });
            group.add_action(&action);
            if from_menu {
                self.fix_match.replace(Some(action));
            }
        }
        let watched = gio::SimpleAction::new(
            "watched",
            Some(&glib::VariantType::new("(sb)").expect("a type")),
        );
        watched.connect_activate(move |_, arg| {
            let Some(hub) = hub.upgrade() else { return };
            if let Some((episode, on)) = arg.and_then(<(String, bool)>::from_variant) {
                hub.send(Command::SetWatched { id, episodes: vec![episode], watched: on });
            }
        });
        group.add_action(&watched);
        // Show synopsis, from a protected row's menu and for a screen reader.
        let reveal = gio::SimpleAction::new("show-synopsis", Some(glib::VariantTy::STRING));
        reveal.connect_activate({
            let page = self.me.borrow().clone();
            move |_, arg| {
                if let (Some(page), Some(number)) =
                    (page.upgrade(), arg.and_then(String::from_variant))
                {
                    page.reveal(&number);
                }
            }
        });
        group.add_action(&reveal);
        if let Some(navigation) = self.navigation.upgrade() {
            navigation.insert_action_group("title", Some(&group));
        }
    }

    /// The header block again, when what the title is matched to changed what it offers.
    fn redress(&self) {
        if let Some(entry) = self.ctx.hub.entry(self.id) {
            self.fill(&entry);
        }
    }

    /// The header block and the side column, from what is known of the title.
    fn fill(&self, entry: &Entry) {
        let size = *self.sized.borrow();
        let narrow = size == Size::Narrow;
        clear(&self.head);
        let hub = &self.ctx.hub;
        // The banner, once: its art does not change with the detail.
        if !self.banner_filled.replace(true) {
            let fit = widgets::faded_banner(hub, entry, BAND);
            self.banner.append(&fit);
        }
        let id = entry.id.get();

        let text = gtk::Box::new(gtk::Orientation::Vertical, 6);
        text.set_valign(gtk::Align::End);
        text.set_hexpand(true);
        let title = widgets::show_title(&entry.title, "page");
        text.append(&title);
        self.heading.replace(Some(title.clone()));
        if let Some(other) = entry.secondary.as_deref().or(entry.title_native.as_deref())
            && other != entry.title
        {
            text.append(&widgets::haze(other, ""));
        }
        text.append(&widgets::haze(&say::title_line(entry), "numeric"));
        if let Some(line) = self.dubbed_to.borrow().as_deref() {
            text.append(&widgets::haze(line, "numeric"));
        }

        let item = {
            let rows = self.rows.borrow();
            say::queue_item(entry, (!rows.is_empty()).then_some(rows.as_slice()))
        };
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_margin_top(10);
        // The source could not choose between its titles: choosing is the way on, in Play's place.
        let asked = hub.known.borrow().matches.asked(entry.id).cloned();
        if asked.is_some() {
            let button = gtk::Button::builder().label(t::match_choose()).build();
            button.add_css_class("pill");
            button.add_css_class("suggested-action");
            button.add_css_class("first-focus");
            button.set_action_name(Some("title.choose-match"));
            button.set_hexpand(narrow);
            actions.append(&button);
        }
        let availability = self.availability();
        // Case B: it does not play here and is on a service of the viewer's, so the main action
        // opens that service.
        let on_service = availability
            .as_ref()
            .filter(|a| asked.is_none() && a.case == AvailabilityCase::OnService)
            .and_then(|a| a.main.clone());
        if let Some(main) = &on_service {
            let inside = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let text = gtk::Label::new(Some(&t::title_watch_on(&main.service)));
            text.set_ellipsize(gtk::pango::EllipsizeMode::End);
            inside.append(&text);
            inside.append(&gtk::Image::from_icon_name("adw-external-link-symbolic"));
            let button = gtk::Button::builder().child(&inside).build();
            button.add_css_class("pill");
            button.add_css_class("suggested-action");
            button.add_css_class("first-focus");
            button.set_action_name(Some("win.licensed-open"));
            button.set_action_target_value(Some(&licensed::target(
                self.id,
                &main.service,
                &main.url,
                None,
            )));
            button.set_hexpand(narrow);
            actions.append(&button);
        }
        match item.button.clone().filter(|_| asked.is_none() && on_service.is_none()) {
            Some(label) => {
                let label = if entry.is_demo {
                    t::title_play_part(&item.episode.to_string())
                } else {
                    label
                };
                // A longer translation is cut at the end, never wider than the window.
                let text = gtk::Label::new(Some(&label));
                text.set_ellipsize(gtk::pango::EllipsizeMode::End);
                let button = gtk::Button::builder().child(&text).build();
                button.add_css_class("pill");
                button.add_css_class("suggested-action");
                button.add_css_class("first-focus");
                button.set_action_name(Some("win.play"));
                button.set_action_target_value(Some(
                    &(id, item.episode.to_string()).to_variant(),
                ));
                button.set_hexpand(narrow);
                actions.append(&button);
                // A controller opened the page: the main action takes focus once it exists.
                if self.ctx.controller_mode.get()
                    && GtkWindowExt::focus(&self.ctx.window)
                        .is_none_or(|f| !f.is_ancestor(&self.head))
                {
                    let button = button.clone();
                    glib::idle_add_local_once(move || {
                        button.grab_focus();
                    });
                }
            }
            None if asked.is_some() => {}
            None => {
                if let Some(waiting) = item.waiting.clone().filter(|_| on_service.is_none()) {
                    actions.append(&widgets::haze(&waiting, ""));
                }
            }
        }
        // A choice each, checked where the title is.
        self.status.set_state(
            &entry.list_status.map_or_else(String::new, |s| s.label().to_owned()).to_variant(),
        );
        let status_menu = gio::Menu::new();
        for segment in LibrarySegment::ALL {
            status_menu.append(
                Some(&super::library::segment_title(segment)),
                Some(&format!("title.status::{}", segment.label())),
            );
        }
        let status = gtk::MenuButton::builder()
            .label(
                entry
                    .list_status
                    .map_or_else(t::library_add_to_list, super::library::segment_title),
            )
            .menu_model(&status_menu)
            .valign(gtk::Align::Center)
            .build();
        status.add_css_class("pill");
        let more_menu = gio::Menu::new();
        let first = gio::Menu::new();
        let (playable, unsourced) = {
            let rows = self.rows.borrow();
            (
                !rows.is_empty() && !rows.iter().any(|r| r.metadata_only),
                choices::next(entry, Some(rows.as_slice())).unsourced,
            )
        };
        let on_tv =
            playable && item.button.is_some() && asked.is_none() && self.ctx.tv.available();
        self.tv_button.set_visible(on_tv);
        if on_tv {
            let from = item.resume.as_ref().map_or(0.0, |r| r.position);
            // The target first, then the name: a name alone is an action asked with no target,
            // which GTK warns about.
            self.tv_button.set_action_target_value(Some(
                &(id, item.episode.to_string(), from).to_variant(),
            ));
            self.tv_button.set_action_name(Some("win.play-on-tv"));
        }
        // A party plays the episode the primary action would.
        if item.button.is_some() && asked.is_none() {
            first.append(
                Some(&t::party_open()),
                Some(&format!("win.watch-together((uint32 {id}, '{}'))", item.episode)), // i18n-skip
            );
        }
        if playable {
            if !unsourced {
                first.append(
                    Some(&t::episode_download_n(i64::from(item.episode))),
                    Some(&format!("win.download((uint32 {id}, '{}'))", item.episode)), // i18n-skip
                );
            }
            if unwatched(&self.rows.borrow()).len() > 1 {
                first.append(
                    Some(&t::episode_download_unwatched()),
                    Some("title.download-unwatched"),
                );
            }
        }
        first.append(Some(&t::episode_mark_previous_watched()), Some("title.mark-previous"));
        more_menu.append_section(None, &first);
        if let Some(availability) = &availability
            && !availability.yours.is_empty()
        {
            let services = gio::Menu::new();
            for link in &availability.yours {
                let item =
                    gio::MenuItem::new(Some(&t::title_watch_on_item(&link.service)), None);
                item.set_action_and_target_value(
                    Some("win.licensed-open"),
                    Some(&licensed::target(self.id, &link.service, &link.url, None)),
                );
                services.append_item(&item);
            }
            more_menu.append_section(None, &services);
        }
        let second = gio::Menu::new();
        if entry.trailer_url.is_some() {
            second.append(Some(&t::title_play_trailer()), Some("title.trailer"));
        }
        second.append(
            Some(&t::title_open_anilist()),
            Some(&format!("win.open-anilist(uint32 {id})")), // i18n-skip
        );
        more_menu.append_section(None, &second);
        let third = gio::Menu::new();
        third.append(Some(&t::source_picker_title()), Some("title.source-picker"));
        third.append(Some(&t::match_fix()), Some("title.fix-match"));
        // Fixing a match needs a source to match on.
        if let Some(action) = self.fix_match.borrow().as_ref() {
            action.set_enabled(hub.known.borrow().can_fix_match());
        }
        third.append(Some(&t::show_settings_title()), Some("title.show-settings"));
        more_menu.append_section(None, &third);
        let more = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .tooltip_text(t::action_more())
            .menu_model(&more_menu)
            .valign(gtk::Align::Center)
            .build();
        more.add_css_class("circular");
        if !narrow {
            actions.append(&status);
            actions.append(&more);
        }
        text.append(&actions);
        if let Some(asked) = &asked {
            // How many titles could be it, under the action.
            text.append(&widgets::haze(&matching::uncertain(asked), ""));
        }
        if narrow {
            let menus = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            menus.append(&status);
            menus.append(&more);
            text.append(&menus);
        }
        if let Some(availability) = &availability {
            let lines = licensed::lines(&self.ctx, availability);
            if !lines.is_empty() {
                let under = gtk::Box::new(gtk::Orientation::Vertical, 0);
                under.set_margin_top(4);
                for line in lines {
                    under.append(&line);
                }
                text.append(&under);
            }
        }

        let line = gtk::Box::new(gtk::Orientation::Horizontal, 20);
        line.set_margin_top(18);
        if !narrow {
            line.append(&widgets::cover(hub, entry, 120, 180));
        }
        line.append(&text);
        self.head.append(&line);

        clear(&self.synopsis);
        if !entry.synopsis.is_empty() {
            let synopsis = gtk::Label::new(Some(&entry.synopsis));
            synopsis.set_wrap(true);
            synopsis.set_xalign(0.0);
            synopsis.set_margin_top(6);
            synopsis.set_max_width_chars(90);
            // Eight lines, then "More" opens the rest in place, only where the text is cut.
            synopsis.set_lines(8);
            synopsis.set_ellipsize(gtk::pango::EllipsizeMode::End);
            self.synopsis.append(&synopsis);
            let more = gtk::Button::with_label(&t::action_more());
            more.add_css_class("flat");
            more.set_halign(gtk::Align::Start);
            more.set_visible(false);
            more.connect_clicked({
                let synopsis = synopsis.clone();
                move |more| {
                    synopsis.set_lines(-1);
                    synopsis.set_ellipsize(gtk::pango::EllipsizeMode::None);
                    more.set_visible(false);
                }
            });
            synopsis.add_tick_callback({
                let more = more.clone();
                move |label, _| {
                    if label.width() == 0 {
                        return gtk::glib::ControlFlow::Continue;
                    }
                    more.set_visible(label.layout().is_ellipsized());
                    gtk::glib::ControlFlow::Break
                }
            });
            self.synopsis.append(&more);
        }
        // The work's credit and licence, wherever it shows as a title.
        if entry.is_demo {
            let credit = super::demo::credit(&self.ctx, &kaeri_model::sources::demo_info());
            credit.set_margin_top(12);
            self.synopsis.append(&credit);
        }

        self.fill_seasons(entry);
        self.fill_side(entry, size);
    }

    /// The franchise's seasons as a picker, when there is more than one.
    fn fill_seasons(&self, entry: &Entry) {
        clear(&self.seasons);
        let Some(seasons) = entry.detail.as_ref().and_then(|d| d.seasons.clone()) else {
            return;
        };
        if seasons.len() < 2 {
            return;
        }
        let names: Vec<String> =
            (1..=seasons.len()).map(|n| t::season_number(n as i64)).collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let picker = gtk::DropDown::from_strings(&refs);
        picker.set_tooltip_text(Some(&t::filter_season()));
        if let Some(at) = seasons.iter().position(|s| s.id == entry.id) {
            picker.set_selected(at as u32);
        }
        picker.connect_selected_notify(move |picker| {
            if let Some(season) = seasons.get(picker.selected() as usize)
                && season.id != AnilistId::new(0)
            {
                let _ = picker
                    .activate_action("win.open-title", Some(&season.id.get().to_variant()));
            }
        });
        self.seasons.append(&picker);
    }

    /// Details, and Related as small covers.
    fn fill_side(&self, entry: &Entry, size: Size) {
        clear(&self.side);
        let details = gtk::Box::new(gtk::Orientation::Vertical, 6);
        details.append(&widgets::heading(&t::title_details()));
        let mut facts: Vec<(String, String)> = Vec::new();
        if let Some(source) = entry.source {
            facts.push((t::title_based_on(), say::material(source)));
        }
        if let Some(line) = say::airing_fact(entry) {
            facts.push((t::filter_status(), line));
        }
        if let Some(made) = kaeri_model::present::made_in(entry) {
            facts.push((t::filter_country(), made));
        }
        if !entry.genres.is_empty() {
            facts.push((
                t::filter_genres(),
                entry.genres.iter().map(|g| say::genre(g)).collect::<Vec<_>>().join(" · "),
            ));
        }
        for (name, value) in facts {
            let line = gtk::Box::new(gtk::Orientation::Vertical, 0);
            line.append(&widgets::haze(&name, "caption"));
            let value = gtk::Label::new(Some(&value));
            value.set_xalign(0.0);
            value.set_wrap(true);
            line.append(&value);
            details.append(&line);
        }
        let more = gtk::Button::with_label(&t::title_more_about());
        more.add_css_class("pill");
        more.set_halign(gtk::Align::Start);
        more.set_margin_top(6);
        more.set_sensitive(entry.detail.is_some());
        more.connect_clicked({
            let ctx = Rc::downgrade(&self.ctx);
            let id = entry.id;
            move |_| {
                let Some(ctx) = ctx.upgrade() else { return };
                if let Some(entry) = ctx.hub.entry(id) {
                    crate::pages::more_about::open(&ctx, &entry);
                }
            }
        });
        details.append(&more);
        self.side.append(&details);

        let related: Vec<LinkedTitle> = entry
            .detail
            .as_ref()
            .map(|d| {
                d.relations
                    .iter()
                    .filter(|r| {
                        !matches!(
                            r.relation,
                            Some(
                                RelationKind::Adaptation
                                    | RelationKind::Source
                                    | RelationKind::Character
                            )
                        )
                    })
                    .take(8)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        if related.is_empty() {
            return;
        }
        let box_ = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .min_children_per_line(if size == Size::Wide { 3 } else { 4 })
            .max_children_per_line(if size == Size::Wide { 3 } else { 8 })
            .column_spacing(8)
            .row_spacing(8)
            .halign(gtk::Align::Start)
            .build();
        for linked in &related {
            let mut shown = Entry::new(linked.id, linked.title.clone());
            shown.cover_url.clone_from(&linked.cover_url);
            shown.cover_color.clone_from(&linked.cover_color);
            let tile = gtk::Box::new(gtk::Orientation::Vertical, 4);
            tile.append(&widgets::cover(&self.ctx.hub, &shown, 76, 114));
            let label = widgets::haze(
                &linked.relation.map_or_else(|| linked.title.clone(), say::relation),
                "caption",
            );
            label.set_wrap(false);
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            label.set_max_width_chars(10);
            tile.append(&label);
            let id = linked.id.get();
            let button = gtk::Button::builder().child(&tile).build();
            button.add_css_class("flat");
            button.add_css_class("related");
            button.set_tooltip_text(Some(&linked.title));
            button.set_action_name(Some("win.open-title"));
            button.set_action_target_value(Some(&id.to_variant()));
            let relation = linked.relation.map(say::relation);
            button.update_property(&[gtk::accessible::Property::Label(&match relation {
                Some(relation) => format!("{}, {relation}", linked.title), // i18n-skip
                None => linked.title.clone(),
            })]);
            widgets::title_menu(&button, id, linked.list_status.is_none());
            crate::trailer::hover_preview(&self.ctx.hub, &button, id);
            box_.append(&button);
        }
        let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
        column.append(&widgets::heading(&t::title_related()));
        column.append(&box_);
        self.side.append(&column);
    }

    /// Whether spoiler protection is on for this show: what it sets for itself, else Settings.
    fn protection_on(&self) -> bool {
        self.own_spoilers.get().unwrap_or_else(|| {
            self.ctx
                .hub
                .known
                .borrow()
                .setting(SettingKey::SpoilerProtection)
                .is_some_and(|r| catalog::is_on(&r))
        })
    }

    /// The next episode: the last one protection shows.
    fn next(&self) -> i64 {
        self.ctx.hub.entry(self.id).and_then(|e| e.progress).map_or(1, |p| i64::from(p.next))
    }

    /// Whether the viewer asked to see this episode through spoiler protection.
    fn shown(&self, number: &str) -> bool {
        self.reveal_all.get() || self.revealed.borrow().contains(number)
    }

    /// What the list is made of: the range showing, the folded rows, where protection starts.
    fn plan(&self, rows: &[EpisodeRow]) -> (Vec<EpisodeRange>, synopsis::Plan) {
        let now = glib::DateTime::now_utc().map_or(0, |t| t.to_unix());
        let ranges = episodes::episode_ranges(rows, now);
        let at = self
            .range
            .get()
            .filter(|at| *at < ranges.len())
            .unwrap_or_else(|| episodes::episode_range_to_open(rows) as usize)
            .min(ranges.len().saturating_sub(1));
        let range = ranges.get(at).copied().unwrap_or(EpisodeRange {
            start: 0,
            end: rows.len() as u32,
            watched: false,
            not_out: false,
        });
        let fold = episodes::episode_fold(rows, &range);
        let plan = synopsis::plan(rows, Some(&range), fold, self.unfolded.get());
        (ranges, plan)
    }

    /// The range picker beside the heading: shown from 101 episodes, with what each range holds.
    fn fill_ranges(&self, rows: &[EpisodeRow], ranges: &[EpisodeRange], at: usize) {
        clear(&self.range_slot);
        if ranges.is_empty() {
            return;
        }
        let items = synopsis::range_items(rows, ranges);
        let model = gtk::StringList::new(&[]);
        for item in &items {
            model.append(&item.label);
        }
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, object| {
            let Some(item) = object.downcast_ref::<gtk::ListItem>() else { return };
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            let label = gtk::Label::new(None);
            label.set_xalign(0.0);
            label.set_hexpand(true);
            let state = widgets::haze("", "caption");
            state.set_wrap(false);
            line.append(&label);
            line.append(&state);
            item.set_child(Some(&line));
        });
        factory.connect_bind({
            let labels: Vec<(String, Option<String>)> =
                items.iter().map(|i| (i.label.clone(), i.state.clone())).collect();
            move |_, object| {
                let Some(item) = object.downcast_ref::<gtk::ListItem>() else { return };
                let Some(line) = item.child().and_downcast::<gtk::Box>() else { return };
                let Some((label, state)) = labels.get(item.position() as usize) else { return };
                if let Some(first) = line.first_child().and_downcast::<gtk::Label>() {
                    first.set_label(label);
                }
                if let Some(last) = line.last_child().and_downcast::<gtk::Label>() {
                    last.set_label(state.as_deref().unwrap_or(""));
                    // The button shows the label alone; the open list says what each holds.
                    last.set_visible(
                        state.is_some()
                            && item.child().is_some_and(|c| {
                                c.ancestor(gtk::Popover::static_type()).is_some()
                            }),
                    );
                }
            }
        });
        let picker = gtk::DropDown::new(Some(model), gtk::Expression::NONE);
        picker.set_factory(Some(&factory));
        picker.set_list_factory(Some(&factory));
        picker.set_tooltip_text(Some(&t::episode_range_label()));
        picker.update_property(&[gtk::accessible::Property::Label(&t::episode_range_label())]);
        picker.set_selected(at as u32);
        let page = self.me.borrow().clone();
        picker.connect_selected_notify(move |picker| {
            if let Some(page) = page.upgrade() {
                page.switch_range(picker.selected() as usize);
            }
        });
        self.range_slot.append(&picker);
    }

    /// A range was chosen: the old list and the new crossfade in place, and the new range opens
    /// at its first unwatched episode with the table laid out again.
    fn switch_range(&self, to: usize) {
        if self.range.get().is_some_and(|at| at == to) {
            return;
        }
        let before =
            gtk::WidgetPaintable::new(Some(&self.episodes_state.widget)).current_image();
        let ghost = gtk::Picture::for_paintable(&before);
        ghost.set_can_target(false);
        ghost.set_halign(gtk::Align::Fill);
        ghost.set_valign(gtk::Align::Start);
        ghost.set_content_fit(gtk::ContentFit::Fill);
        self.range.set(Some(to));
        self.unfolded.set(false);
        // The next layout decides the rows' room again.
        self.outlook.set(None);
        self.draw_episodes();
        self.veil.add_overlay(&ghost);
        let veil = self.veil.clone();
        let gone = ghost.clone();
        let fade = adw::TimedAnimation::new(&ghost, 1.0, 0.0, 300, motion::opacity(&ghost));
        fade.set_easing(adw::Easing::EaseOutCubic);
        fade.connect_done(move |_| veil.remove_overlay(&gone));
        fade.play();
        // The page scrolls the pickers into view first when they were above the screen.
        if let Some(parent) = self.range_slot.parent() {
            let y = parent
                .compute_point(
                    &self.scroll.child().unwrap_or_else(|| parent.clone()),
                    &gtk::graphene::Point::zero(),
                )
                .map_or(0.0, |p| f64::from(p.y()));
            let adjustment = self.scroll.vadjustment();
            if adjustment.value() > y {
                adjustment.set_value(y);
            }
        }
    }

    /// The episode table, drawn whole: when it arrives, when the window changes size and when the
    /// list is made of something else (a range, an opened fold, a reveal of every row).
    fn draw_episodes(&self) {
        clear_list(&self.episodes);
        self.lines.borrow_mut().clear();
        self.protection.replace(None);
        let rows = self.rows.borrow();
        if rows.is_empty() {
            self.drawn.replace(None);
            self.fill_ranges(&rows, &[], 0);
            if self.listed.get() {
                self.episodes_state.empty();
            }
            return;
        }
        let narrow = *self.sized.borrow() == Size::Narrow;
        let outlook = self.outlook.get().unwrap_or_else(|| episodes::episode_text(&rows));
        self.outlook.set(Some(outlook));
        let (ranges, plan) = self.plan(&rows);
        let selected = ranges.iter().position(|r| r.start as usize == plan.start).unwrap_or(0);
        self.range.set(if ranges.is_empty() { None } else { Some(selected) });
        self.fill_ranges(&rows, &ranges, selected);

        let on = self.protection_on();
        let next = self.next();
        let entry = self.ctx.hub.entry(self.id);
        let color = entry.as_ref().and_then(|e| e.cover_color.clone());
        // A row without a still of its own stands on the show's wide art, never blurred.
        let wide_art =
            entry.as_ref().and_then(|e| e.banner_url.clone().or(e.cover_url.clone()));
        let id = self.id.get();
        let shown = &rows[plan.start + plan.fold..plan.end];
        let protection_at = synopsis::protection_start(shown, on, next).map(|(at, _)| at);
        self.drawn.replace(Some(Drawn {
            numbers: rows.iter().map(|r| r.number.clone()).collect(),
            narrow,
            range: self.range.get(),
            fold: plan.fold,
            protection_at,
        }));
        let mut lines = Vec::with_capacity(shown.len());
        // Case B: each episode opens its own page on the service, or else the title's.
        let service = self
            .availability()
            .filter(|a| a.case == AvailabilityCase::OnService)
            .and_then(|a| a.main.clone().map(|main| (main, a.episodes)));

        if plan.fold > 0 {
            let first = synopsis::number(&rows[plan.start].number).unwrap_or(1);
            let last =
                synopsis::number(&rows[plan.start + plan.fold - 1].number).unwrap_or(first);
            let label = t::title_watched_range(first, last);
            let button = gtk::Button::with_label(&label);
            button.add_css_class("flat");
            button.set_halign(gtk::Align::Start);
            button.update_property(&[gtk::accessible::Property::Label(&format!(
                "{label}, {}", // i18n-skip
                t::state_collapsed().to_lowercase()
            ))]);
            let page = self.me.borrow().clone();
            button.connect_clicked(move |_| {
                if let Some(page) = page.upgrade() {
                    page.unfold();
                }
            });
            self.episodes.append(&Self::note_row(&button));
        }

        for (at, row) in shown.iter().enumerate() {
            if protection_at == Some(at) {
                let (_, before) =
                    synopsis::protection_start(shown, on, next).unwrap_or((at, 0));
                self.episodes.append(&self.protection_row(before, narrow));
            }
            let held = synopsis::protected(row, on, next, self.shown(&row.number));
            let room = synopsis::has_room(outlook, held);
            let height = synopsis::row_height(narrow, room);
            let tall = narrow && room;

            let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
            root.add_css_class("episode-row");
            root.add_css_class("synopsis-row");
            root.set_size_request(-1, height);
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 14);

            let w = if narrow { synopsis::STILL_NARROW } else { synopsis::STILL_WIDE };
            let own = row.thumbnail.clone();
            let (still, _) = widgets::still(
                &self.ctx.hub,
                own.as_deref().or(wide_art.as_deref()),
                color.as_deref(),
                w,
            );
            // Sharp under blurred: a reveal fades the blurred one away.
            let (still_widget, blur): (gtk::Widget, Option<gtk::Widget>) =
                if own.is_some() && synopsis::protectable(row, on, next) {
                    let (veil, veil_frame) =
                        widgets::still(&self.ctx.hub, own.as_deref(), color.as_deref(), w);
                    veil_frame.add_css_class("spoiler");
                    veil.set_can_target(false);
                    let both = gtk::Overlay::new();
                    both.set_child(Some(&still));
                    both.add_overlay(&veil);
                    both.set_valign(gtk::Align::Center);
                    veil.set_opacity(if held { 1.0 } else { 0.0 });
                    (both.upcast(), Some(veil.upcast()))
                } else {
                    (still.upcast(), None)
                };
            line.append(&still_widget);

            let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
            text.set_hexpand(true);
            let name = match &row.title {
                Some(title) if !title.is_empty() => {
                    t::episode_with_title(say::episode_n(&row.number), title)
                }
                _ => t::episode_number(say::episode_n(&row.number)),
            };
            let label = gtk::Label::new(Some(&name));
            label.add_css_class("heading");
            label.set_xalign(0.0);
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            if narrow {
                // The name has two lines at the narrow width, where one cut it short.
                label.set_wrap(true);
                label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                label.set_lines(2);
            }
            text.append(&label);
            let sub = widgets::haze("", "caption");
            sub.add_css_class("numeric");
            text.append(&sub);
            let discuss = gtk::Button::new();
            discuss.add_css_class("flat");
            discuss.add_css_class("caption");
            discuss.set_halign(gtk::Align::Start);
            discuss.set_visible(false);
            discuss.set_action_name(Some("win.open-discussion"));
            discuss.set_action_target_value(Some(&(id, row.number.clone()).to_variant()));
            text.append(&discuss);
            let bar = gtk::ProgressBar::new();
            bar.set_margin_top(4);
            // Narrow, the track is as wide as the room the title leaves.
            bar.set_size_request(if narrow { 48 } else { 120 }, -1);
            bar.set_halign(if narrow { gtk::Align::Fill } else { gtk::Align::Start });
            crate::widgets::name_progress(&bar);
            text.append(&bar);

            // Three lines of synopsis in a box of fixed height. Beside the still the box is
            // always there, so a reveal moves nothing; under it, only where the row is tall.
            let synopsis_label = gtk::Label::new(None);
            synopsis_label.add_css_class("haze");
            synopsis_label.set_xalign(0.0);
            synopsis_label.set_yalign(0.0);
            synopsis_label.set_wrap(true);
            synopsis_label.set_wrap_mode(gtk::pango::WrapMode::Word);
            synopsis_label.set_lines(3);
            synopsis_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            synopsis_label.set_hexpand(true);
            let room_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
            room_box.set_size_request(-1, synopsis::TEXT_HEIGHT);
            room_box.set_overflow(gtk::Overflow::Hidden);
            room_box.append(&synopsis_label);
            room_box.set_margin_top(4);
            if narrow {
                room_box.set_visible(tall);
            } else {
                text.append(&room_box);
            }

            if narrow {
                text.set_valign(gtk::Align::Center);
                line.set_valign(if tall { gtk::Align::Start } else { gtk::Align::Center });
                line.set_vexpand(!tall);
                if tall {
                    line.set_margin_top(8);
                }
            } else {
                // Title and facts stay at the still's top, whether or not text is below them.
                text.set_valign(gtk::Align::Start);
                text.set_margin_top(10);
                line.set_valign(gtk::Align::Fill);
                line.set_vexpand(true);
            }
            line.append(&text);

            let check = gtk::CheckButton::new();
            check.set_valign(gtk::Align::Center);
            check.update_property(&[gtk::accessible::Property::Label(
                &t::title_linux_episode_watched(say::episode_n(&row.number)),
            )]);
            let episode = row.number.clone();
            let toggled = check.connect_toggled(move |check| {
                let _ = check.activate_action(
                    "title.watched",
                    Some(&(episode.clone(), check.is_active()).to_variant()),
                );
            });
            line.append(&check);

            let mut outside: Option<glib::Variant> = None;
            if let Some((main, listings)) = &service {
                let number = row.number.parse::<u32>().ok();
                let own = number.and_then(|n| listings.iter().find(|l| l.episode == n));
                let url = own.map_or(&main.url, |l| &l.url);
                let target = licensed::target(self.id, &main.service, url, number);
                let glyph = gtk::Image::from_icon_name("adw-external-link-symbolic");
                glyph.set_valign(gtk::Align::Center);
                glyph.add_css_class("dim-label");
                glyph.set_tooltip_text(Some(&t::licensed_opens_outside()));
                line.append(&glyph);
                outside = Some(target);
            }

            let menu = gio::Menu::new();
            let more = gtk::MenuButton::builder()
                .icon_name("view-more-symbolic")
                .tooltip_text(t::episode_options())
                .menu_model(&menu)
                .valign(gtk::Align::Center)
                .build();
            more.add_css_class("flat");
            line.append(&more);
            root.append(&line);
            if narrow {
                room_box.set_margin_start(0);
                room_box.set_margin_bottom(8);
                root.append(&room_box);
            }

            let list_row = gtk::ListBoxRow::builder()
                .child(&root)
                .activatable(choices::playable(row) || outside.is_some())
                .build();
            match &outside {
                Some(target) => {
                    list_row.set_action_name(Some("win.licensed-open"));
                    list_row.set_action_target_value(Some(target));
                }
                None => {
                    list_row.set_action_name(Some("win.play"));
                    list_row
                        .set_action_target_value(Some(&(id, row.number.clone()).to_variant()));
                }
            }
            widgets::context_menu(&list_row, &menu);
            self.episodes.append(&list_row);

            let drawn = EpisodeLine {
                number: row.number.clone(),
                check,
                toggled,
                name: label,
                still: still_widget,
                root,
                sub,
                discuss,
                bar,
                menu,
                shown: RefCell::new(row.clone()),
                list_row,
                synopsis: synopsis_label,
                room_box,
                blur,
                protected: Cell::new(held),
                room: Cell::new(room),
                height: Cell::new(height),
            };
            self.dress(&drawn, row, false);
            lines.push(drawn);
        }
        drop(rows);
        self.lines.replace(lines);
        self.episodes_state.content();
    }

    /// A line of the list that is no episode: not a row to play, only its button.
    fn note_row(button: &impl IsA<gtk::Widget>) -> gtk::ListBoxRow {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        line.set_margin_top(6);
        line.set_margin_bottom(6);
        line.set_margin_start(6);
        line.append(button);
        gtk::ListBoxRow::builder().child(&line).activatable(false).selectable(false).build()
    }

    /// The one line where spoiler protection starts, with its button: Show, or Hide once the
    /// rows are shown.
    fn protection_row(&self, before: i64, narrow: bool) -> gtk::ListBoxRow {
        let label = gtk::Label::new(None);
        label.add_css_class("haze");
        label.set_xalign(0.0);
        label.set_hexpand(true);
        label.set_wrap(true);
        let button = gtk::Button::new();
        button.add_css_class("flat");
        button.set_valign(gtk::Align::Center);
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        line.set_margin_top(8);
        line.set_margin_bottom(8);
        line.set_margin_start(6);
        line.set_margin_end(6);
        line.append(&label);
        line.append(&button);
        let page = self.me.borrow().clone();
        button.connect_clicked(move |_| {
            if let Some(page) = page.upgrade() {
                page.reveal_every(!page.reveal_all.get());
            }
        });
        self.protection.replace(Some(Protection { label, button, episode: before }));
        self.say_protection(narrow);
        gtk::ListBoxRow::builder().child(&line).activatable(false).selectable(false).build()
    }

    /// The protection line's words, for whether the rows are showing.
    fn say_protection(&self, narrow: bool) {
        let Some(line) = self
            .protection
            .borrow()
            .as_ref()
            .map(|p| (p.label.clone(), p.button.clone(), p.episode))
        else {
            return;
        };
        let (label, button, episode) = line;
        let showing = self.reveal_all.get();
        label.set_label(&if showing {
            t::episode_protection_shown(episode)
        } else if narrow {
            t::episode_protection_line_short(episode)
        } else {
            t::episode_protection_line(episode)
        });
        button.set_label(&if showing {
            t::episode_protection_hide()
        } else {
            t::episode_protection_show()
        });
    }

    /// The folded watched episodes were opened: they appear above, and the next episode's row
    /// stays where it was on screen.
    fn unfold(&self) {
        let next = self.lines.borrow().first().map(|l| l.list_row.clone());
        let before = next.as_ref().and_then(|row| self.row_top(row));
        self.unfolded.set(true);
        self.draw_episodes();
        if let Some(before) = before {
            let page = self.me.borrow().clone();
            let number =
                self.rows.borrow().iter().find(|r| !r.completed).map(|r| r.number.clone());
            glib::idle_add_local_once(move || {
                let Some(page) = page.upgrade() else { return };
                let target = page
                    .lines
                    .borrow()
                    .iter()
                    .find(|l| Some(&l.number) == number.as_ref())
                    .map(|l| l.list_row.clone());
                if let Some(now) = target.and_then(|row| page.row_top(&row)) {
                    let adjustment = page.scroll.vadjustment();
                    adjustment.set_value(adjustment.value() + now - before);
                }
            });
        }
    }

    /// Where a row's top is, in the scrolled page's content.
    fn row_top(&self, row: &gtk::ListBoxRow) -> Option<f64> {
        let content = self.scroll.child()?;
        row.compute_point(&content, &gtk::graphene::Point::zero()).map(|p| f64::from(p.y()))
    }

    /// Show synopsis on one protected episode: its still and its text, for this visit.
    fn reveal(&self, number: &str) {
        self.revealed.borrow_mut().insert(number.to_owned());
        self.settle_all(Some(number.to_owned()));
    }

    /// The protection line's button: every row after it shows (or is held back again).
    fn reveal_every(&self, on: bool) {
        self.reveal_all.set(on);
        if !on {
            self.revealed.borrow_mut().clear();
        }
        self.say_protection(*self.sized.borrow() == Size::Narrow);
        self.settle_all(None);
    }

    /// Every row (or the one `only` names) takes the state the rules give it now.
    fn settle_all(&self, only: Option<String>) {
        let rows = self.rows.borrow().clone();
        for line in self.lines.borrow().iter() {
            if only.as_ref().is_some_and(|n| *n != line.number) {
                continue;
            }
            if let Some(row) = rows.iter().find(|r| r.number == line.number) {
                self.settle(line, row, true);
            }
        }
    }

    /// One row's protection, room and text as the rules give them now: a blurred still fading to
    /// sharp, a narrow row taking its height before its text fades in.
    fn settle(&self, line: &EpisodeLine, row: &EpisodeRow, animate: bool) {
        let outlook = self.outlook.get().unwrap_or(EpisodeText::None);
        let narrow = *self.sized.borrow() == Size::Narrow;
        let on = self.protection_on();
        let next = self.next();
        let held = synopsis::protected(row, on, next, self.shown(&row.number));
        let was_held = line.protected.replace(held);
        line.protected.set(held);
        if let Some(blur) = &line.blur {
            let to = if held { 1.0 } else { 0.0 };
            if animate && was_held != held && (blur.opacity() - to).abs() > f64::EPSILON {
                let from = blur.opacity();
                let animation = if held {
                    motion::quick(blur, from, to, motion::opacity(blur))
                } else {
                    motion::standard(blur, from, to, motion::opacity(blur))
                };
                animation.play();
            } else {
                blur.set_opacity(to);
            }
        }
        let room = synopsis::has_room(outlook, held);
        line.room.set(room);
        let height = synopsis::row_height(narrow, room);
        let old = line.height.replace(height);
        if !narrow || height == old {
            // Beside the still the row keeps its height: only the text changes.
            self.set_text(line, row, animate);
            return;
        }
        // Under the still the row takes its height first, and the text fades in once there is
        // room for it, so no line is ever clipped by a growing frame.
        line.room_box.set_visible(room || animate);
        if !animate {
            line.root.set_size_request(-1, height);
            line.room_box.set_visible(room);
            self.set_text(line, row, false);
            return;
        }
        if !room {
            self.set_text(line, row, false);
        }
        let root = line.root.clone();
        let target = adw::CallbackAnimationTarget::new(move |v| {
            root.set_size_request(-1, v.round() as i32);
        });
        let animation = motion::standard(&line.root, f64::from(old), f64::from(height), target);
        let page = self.me.borrow().clone();
        let number = line.number.clone();
        let box_ = line.room_box.clone();
        animation.connect_done(move |_| {
            box_.set_visible(room);
            if room && let Some(page) = page.upgrade() {
                page.set_text_by_number(&number, true);
            }
        });
        animation.play();
    }

    fn set_text_by_number(&self, number: &str, animate: bool) {
        let rows = self.rows.borrow().clone();
        for line in self.lines.borrow().iter().filter(|l| l.number == number) {
            if let Some(row) = rows.iter().find(|r| r.number == number) {
                self.set_text(line, row, animate);
            }
        }
    }

    /// The row's synopsis, its tooltip and what a screen reader says of it. Text that arrives
    /// fades in over 0.2 s into the room the row kept; a row laid out without room waits for
    /// the next layout.
    fn set_text(&self, line: &EpisodeLine, row: &EpisodeRow, animate: bool) {
        let text = synopsis::text_of(row).filter(|_| line.room.get() && !line.protected.get());
        let now = line.synopsis.label();
        match text {
            Some(text) if now != text => {
                line.synopsis.set_label(text);
                if animate {
                    motion::fade_in(&line.synopsis);
                } else {
                    line.synopsis.set_opacity(1.0);
                }
            }
            Some(_) => line.synopsis.set_opacity(1.0),
            None => line.synopsis.set_label(""),
        }
        line.list_row.set_tooltip_text(text);
        let description = if line.protected.get() {
            t::episode_synopsis_hidden()
        } else {
            text.unwrap_or_default().to_owned()
        };
        line.list_row.update_property(&[gtk::accessible::Property::Description(&description)]);
    }

    /// Marks and progress moved: each row changed in place.
    fn update_episodes(&self) {
        // A table that is made of something else (protection moved with the marks, a longer
        // table) draws the list again; the same table changes its rows where they stand.
        let rows = self.rows.borrow();
        let on = self.protection_on();
        let next = self.next();
        if let Some(drawn) = self.drawn.borrow().as_ref() {
            let (_, plan) = self.plan(&rows);
            let shown = &rows[plan.start + plan.fold..plan.end];
            let numbers_same = drawn.numbers.len() == rows.len()
                && drawn.numbers.iter().zip(rows.iter()).all(|(a, b)| *a == b.number);
            let at = synopsis::protection_start(shown, on, next).map(|(at, _)| at);
            if !numbers_same || drawn.fold != plan.fold || drawn.protection_at != at {
                drop(rows);
                self.draw_episodes();
                return;
            }
        }
        for line in self.lines.borrow().iter() {
            if let Some(row) = rows.iter().find(|r| r.number == line.number)
                && *line.shown.borrow() != *row
            {
                self.dress(line, row, true);
                line.shown.replace(row.clone());
            }
        }
    }

    /// One row as `row` stands: its check, its line, its still and its menu.
    /// Whether the episode offers its discussion: the setting is on, and with spoiler
    /// protection the viewer has watched it.
    fn discussion_offered(&self, row: &EpisodeRow, next: u32, spoilers: bool) -> bool {
        let number: u32 = row.number.parse().unwrap_or(0);
        let on = self
            .ctx
            .hub
            .known
            .borrow()
            .setting(SettingKey::EpisodeDiscussions)
            .is_some_and(|r| catalog::is_on(&r));
        on && number > 0 && !row.metadata_only && (!spoilers || row.completed || number < next)
    }

    /// A discussion was found: the rows that offer it again.
    fn redress_discussions(&self) {
        for line in self.lines.borrow().iter() {
            let row = line.shown.borrow().clone();
            self.dress(line, &row, false);
        }
    }

    fn dress(&self, line: &EpisodeLine, row: &EpisodeRow, animate: bool) {
        let entry = self.ctx.hub.entry(self.id);
        let next = entry.as_ref().and_then(|e| e.progress).map_or(1, |p| p.next);
        let spoilers = self.protection_on();

        line.check.block_signal(&line.toggled);
        line.check.set_active(row.completed);
        line.check.unblock_signal(&line.toggled);
        line.check.set_tooltip_text(Some(&if row.completed {
            t::episode_mark_unwatched()
        } else {
            t::episode_mark_watched()
        }));
        if row.completed {
            line.name.add_css_class("haze");
        } else {
            line.name.remove_css_class("haze");
        }
        let to = if row.completed { WATCHED } else { 1.0 };
        let from = line.still.opacity();
        if animate && (from - to).abs() > f64::EPSILON {
            motion::quick(&line.still, from, to, motion::opacity(&line.still)).play();
        } else {
            line.still.set_opacity(to);
        }

        // An episode no source lists is dimmed and says so; it cannot be played or downloaded.
        if row.unsourced {
            line.root.add_css_class("dim-label");
        } else {
            line.root.remove_css_class("dim-label");
        }
        let part_way = row.watched > 0.0 && !row.completed;
        let now = glib::DateTime::now_utc().map_or(0, |t| t.to_unix());
        let sub = synopsis::facts(row, now, |at| {
            glib::DateTime::from_unix_local(at)
                .ok()
                .and_then(|d| d.format("%-d %b %Y").ok())
                .map_or_else(String::new, |d| d.to_string())
        });
        line.sub.set_label(&sub);
        line.sub.set_visible(!sub.is_empty());
        line.bar.set_fraction(row.watched);
        line.bar.set_visible(part_way);

        let id = self.id.get();
        let n = &row.number;
        let target = format!("(uint32 {id}, '{n}')"); // i18n-skip
        line.menu.remove_all();
        let first = gio::Menu::new();
        if synopsis::protected(row, spoilers, i64::from(next), self.shown(&row.number)) {
            // Show synopsis comes first for a protected episode.
            first.append(
                Some(&t::episode_synopsis_show()),
                Some(&format!("title.show-synopsis('{n}')")), // i18n-skip
            );
        }
        if choices::playable(row) {
            let episode = say::episode_n(n);
            let label = match (part_way, row.duration_secs) {
                (true, Some(d)) => {
                    t::title_button_resume(episode, &say::clock(d as f64 * row.watched))
                }
                _ => t::downloads_play_episode(episode),
            };
            first.append(Some(&label), Some(&format!("win.play({target})")));
            if part_way {
                first.append(
                    Some(&t::episode_play_from_beginning()),
                    Some(&format!("win.play-from-start({target})")),
                );
            }
            if self.ctx.tv.available() {
                // Resumed on the TV where it was left here.
                let from = match (part_way, row.duration_secs) {
                    (true, Some(d)) => d as f64 * row.watched,
                    _ => 0.0,
                };
                first.append(
                    Some(&t::tv_play_on_menu()),
                    Some(&format!("win.play-on-tv((uint32 {id}, '{n}', {from:.1}))")), // i18n-skip
                );
            }
        }
        line.menu.append_section(None, &first);
        let second = gio::Menu::new();
        if row.completed {
            second.append(
                Some(&t::episode_mark_n_unwatched(say::episode_n(n))),
                Some(&format!("win.mark-unwatched({target})")),
            );
        } else {
            second.append(
                Some(&t::episode_mark_n_watched(say::episode_n(n))),
                Some(&format!("win.mark-watched({target})")),
            );
        }
        line.menu.append_section(None, &second);
        let offered = self.discussion_offered(row, next, spoilers);
        if offered {
            let talk = gio::Menu::new();
            talk.append(
                Some(&t::discussion_open()),
                Some(&format!("win.open-discussion({target})")),
            );
            line.menu.append_section(None, &talk);
        }
        let found = row
            .number
            .parse::<u32>()
            .ok()
            .and_then(|n| self.ctx.hub.known.borrow().discussions.get(&(id, n)).copied());
        match found.filter(|_| offered) {
            Some(count) => {
                line.discuss.set_label(&t::discussion_row(i64::from(count)));
                line.discuss.set_visible(true);
            }
            None => line.discuss.set_visible(false),
        }
        if choices::playable(row) {
            let third = gio::Menu::new();
            third.append(
                Some(&t::episode_download_n(say::episode_n(n))),
                Some(&format!("win.download({target})")),
            );
            line.menu.append_section(None, &third);
        }
        // The row is one element: number, title and facts in its label, the synopsis last in its
        // description (set with the text, and not announced when it arrives late).
        let spoken = if sub.is_empty() {
            line.name.label().to_string()
        } else {
            format!("{}, {sub}", line.name.label()) // i18n-skip
        };
        line.list_row.update_property(&[gtk::accessible::Property::Label(&spoken)]);
        self.settle(line, row, animate);
    }
}

/// The episodes Download Unwatched Episodes queues: each one a source lists, out, and not
/// watched.
fn unwatched(rows: &[EpisodeRow]) -> Vec<String> {
    let now = glib::DateTime::now_utc().map_or(0, |t| t.to_unix());
    rows.iter()
        .filter(|r| {
            choices::playable(r) && !r.completed && r.airs_at.is_none_or(|at| at <= now)
        })
        .map(|r| r.number.clone())
        .collect()
}

fn clear(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

fn clear_list(list: &gtk::ListBox) {
    list.remove_all();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_range_download_skips_the_episodes_no_source_lists() {
        let row = |number: &str, unsourced: bool, metadata_only: bool| EpisodeRow {
            number: number.into(),
            title: None,
            thumbnail: None,
            description: None,
            duration_secs: None,
            watched: 0.0,
            completed: false,
            kind: None,
            skippable: false,
            airs_at: None,
            delayed_from: None,
            metadata_only,
            past_progress: false,
            synopsis_pending: false,
            unsourced,
            source: None,
        };
        let rows = [
            row("1", false, false),
            row("2", true, false),
            row("3", false, false),
            row("4", false, true),
        ];
        assert_eq!(unwatched(&rows), ["1", "3"]);
    }
}

#[cfg(test)]
mod stack_tests {
    use super::*;

    #[test]
    fn a_title_already_in_the_stack_is_not_stacked_again() {
        let id = AnilistId::new(900_002);
        let at = |tags: &[&str]| -> Vec<Option<String>> {
            tags.iter().map(|t| Some((*t).to_owned())).collect()
        };
        assert_eq!(reach(&at(&["title-900001"]), &tag(id)), Reach::Push);
        assert_eq!(reach(&at(&["title-900001", "title-900002"]), &tag(id)), Reach::Stay);
        assert_eq!(
            reach(&at(&["title-900002", "title-900001", "title-900003"]), &tag(id)),
            Reach::PopTo
        );
        assert_eq!(reach(&[], &tag(id)), Reach::Push);
    }
}
