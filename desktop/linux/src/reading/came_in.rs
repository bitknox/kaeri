//! What came in after adding a book source: a server, a folder or a plugin store. Each is a
//! dialog with its own header bar (Cancel, the title, the one action), the field it needs, and the
//! engine's answer under it. The answer keeps the waiting rule: nothing for 600 ms, then a
//! waiting line for at least 300 ms, and each state crossfades into the next.

use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    rc::Rc,
    time::{Duration, Instant},
};

use adw::prelude::*;
use gtk::{gio, glib};
use kaeri_i18n::t;
use kaeri_model::{
    book_sources::{BookServerKind, BookSourceCommand, BookSourceEvent, FolderRef},
    came_in::{
        CameIn, FolderReport, NothingCameIn, PluginsInstalled, ServerFound, StoreReport,
    },
    command::Command,
    event::Event,
    medium::{KaeriIsFor, MediumCommand},
    stores::{InvitePreview, InvitedStore, Secret, StorePluginRow, StorePluginStatus},
};

use crate::{
    motion,
    states::{AT_LEAST, SILENT},
    widgets,
    window::{Ctx, Size},
};

fn send(hub: &crate::hub::Hub, command: BookSourceCommand) {
    hub.send(Command::BookSources(command));
}

// ---------------------------------------------------------------------------------------------
// Parts every form shares

/// The waiting rule for one answer: the line shows after 600 ms, and once it has, the answer
/// waits until it has been up for 300 ms.
#[derive(Default, Clone)]
pub struct Wait {
    turn: Rc<Cell<u64>>,
    shown: Rc<Cell<Option<Instant>>>,
}

impl Wait {
    /// Asked: `show` runs once 600 ms go by with no answer.
    pub fn start(&self, show: impl FnOnce() + 'static) {
        let turn = self.turn.get() + 1;
        self.turn.set(turn);
        self.shown.set(None);
        let (at, shown) = (Rc::clone(&self.turn), Rc::clone(&self.shown));
        glib::timeout_add_local_once(SILENT, move || {
            if at.get() == turn {
                shown.set(Some(Instant::now()));
                show();
            }
        });
    }

    /// Answered: `deliver` runs now, or when the line has been up for 300 ms.
    pub fn end(&self, deliver: impl FnOnce() + 'static) {
        let turn = self.turn.get() + 1;
        self.turn.set(turn);
        let Some(since) = self.shown.take() else {
            deliver();
            return;
        };
        let left = AT_LEAST.saturating_sub(since.elapsed());
        if left.is_zero() {
            deliver();
            return;
        }
        let at = Rc::clone(&self.turn);
        glib::timeout_add_local_once(left, move || {
            if at.get() == turn {
                deliver();
            }
        });
    }

    /// Whether the waiting line is up.
    pub fn showing(&self) -> bool {
        self.shown.get().is_some()
    }

    pub fn cancel(&self) {
        self.turn.set(self.turn.get() + 1);
        self.shown.set(None);
    }
}

/// A place whose content crossfades from one state to the next, both drawn as it goes, never
/// through empty.
pub struct Swap {
    pub stack: gtk::Stack,
    count: Cell<u32>,
}

impl Swap {
    pub fn new() -> Rc<Self> {
        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(if motion::reduced() { 0 } else { 200 })
            .vhomogeneous(false)
            .hhomogeneous(false)
            .interpolate_size(true)
            .build();
        Rc::new(Self { stack, count: Cell::new(0) })
    }

    /// Show `child` in place of what shows, which goes once the crossfade is over.
    pub fn show(&self, child: &impl IsA<gtk::Widget>) {
        let n = self.count.get() + 1;
        self.count.set(n);
        let name = format!("s{n}"); // i18n-skip: a stack child's name
        self.stack.add_named(child, Some(&name));
        self.stack.set_visible_child_name(&name);
        let (stack, keep) = (self.stack.clone(), child.clone().upcast::<gtk::Widget>());
        let wait = Duration::from_millis(u64::from(self.stack.transition_duration()) + 60);
        glib::timeout_add_local_once(wait, move || {
            let mut child = stack.first_child();
            while let Some(widget) = child {
                child = widget.next_sibling();
                if widget != keep && stack.visible_child().as_ref() != Some(&widget) {
                    stack.remove(&widget);
                }
            }
        });
    }
}

/// What a note says: found, for information, or failed.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Found,
    Info,
    Failed,
}

/// A note: its glyph, its title in bold and a line under it.
pub fn note(mark: Mark, title: &str, line: Option<&str>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("came-in-note");
    let (icon, class) = match mark {
        Mark::Found => ("object-select-symbolic", "came-in-found"),
        Mark::Info => ("help-about-symbolic", "came-in-info"),
        Mark::Failed => ("dialog-warning-symbolic", "came-in-failed"),
    };
    let glyph = gtk::Image::from_icon_name(icon);
    glyph.add_css_class(class);
    glyph.set_valign(gtk::Align::Start);
    glyph.set_margin_top(2);
    row.append(&glyph);
    let words = gtk::Box::new(gtk::Orientation::Vertical, 2);
    words.set_hexpand(true);
    let heading = gtk::Label::new(Some(title));
    heading.add_css_class("heading");
    heading.set_xalign(0.0);
    heading.set_wrap(true);
    words.append(&heading);
    if let Some(line) = line {
        words.append(&widgets::haze(line, ""));
    }
    row.append(&words);
    row
}

/// The waiting line: a spinner and what is being waited on.
pub fn waiting(text: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let spinner = adw::Spinner::new();
    spinner.set_size_request(16, 16);
    row.append(&spinner);
    row.append(&widgets::haze(text, ""));
    row
}

fn nothing() -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Vertical, 0)
}

/// Where an add shows: its own dialog, or a page pushed inside first run, as the board draws
/// it there ("5 of 5" over its title, the action as the amber pill at the foot).
#[derive(Clone)]
pub enum Host {
    Dialog(adw::Dialog),
    Page { page: adw::NavigationPage, nav: adw::NavigationView },
}

impl Host {
    pub fn widget(&self) -> gtk::Widget {
        match self {
            Self::Dialog(dialog) => dialog.clone().upcast(),
            Self::Page { page, .. } => page.clone().upcast(),
        }
    }

    pub fn close(&self) {
        match self {
            Self::Dialog(dialog) => {
                dialog.close();
            }
            Self::Page { page, nav } => {
                if nav.visible_page().as_ref() == Some(page) {
                    nav.pop();
                }
            }
        }
    }

    fn force_close(&self) {
        match self {
            Self::Dialog(dialog) => {
                dialog.force_close();
            }
            Self::Page { .. } => self.close(),
        }
    }

    /// Runs once when the add goes away: the dialog closed, or the page popped.
    pub fn on_closed(&self, f: impl Fn() + 'static) {
        match self {
            Self::Dialog(dialog) => {
                dialog.connect_closed(move |_| f());
            }
            Self::Page { page, nav } => {
                let handler: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::default();
                let mine = page.downgrade();
                let id = nav.connect_popped({
                    let handler = Rc::clone(&handler);
                    move |nav, popped| {
                        if mine.upgrade().as_ref() == Some(popped) {
                            f();
                            if let Some(id) = handler.borrow_mut().take() {
                                nav.disconnect(id);
                            }
                        }
                    }
                });
                handler.replace(Some(id));
            }
        }
    }

    pub fn present(&self, ctx: &Rc<Ctx>) {
        match self {
            Self::Dialog(dialog) => dialog.present(Some(&ctx.window)),
            Self::Page { page, nav } => nav.push(page),
        }
    }
}

thread_local! {
    /// First run's pages and its step line, while its sources page is up: adds show there.
    static IN_PAGES: RefCell<Option<(adw::NavigationView, String)>> = const { RefCell::new(None) };
}

/// A source that was just added, as first run lists it under its title afterwards.
pub struct Added {
    pub icon: &'static str,
    pub title: String,
    pub line: String,
}

/// Who hears each add while first run is up.
type AddedListener = Rc<dyn Fn(Added)>;

thread_local! {
    static ON_ADDED: RefCell<Option<AddedListener>> = const { RefCell::new(None) };
}

/// Hear each add that went through, while first run is up.
pub fn on_added(f: Option<AddedListener>) {
    ON_ADDED.replace(f);
}

thread_local! {
    /// Servers added in this run, with when: their rows say "added just now" for a while.
    static JUST_ADDED: RefCell<Vec<(String, std::time::Instant)>> = const { RefCell::new(Vec::new()) };
}

/// How long a server's row says it was added just now.
const JUST_NOW: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// Whether the server `id` was added in the last ten minutes.
pub fn just_added(id: &str) -> bool {
    JUST_ADDED
        .with_borrow(|list| list.iter().any(|(at, when)| at == id && when.elapsed() < JUST_NOW))
}

fn mark_added(id: &str) {
    JUST_ADDED.with_borrow_mut(|list| {
        list.retain(|(at, when)| at != id && when.elapsed() < JUST_NOW);
        list.push((id.to_owned(), std::time::Instant::now()));
    });
}

/// For a shot: the server `id` was just added.
#[cfg(feature = "fixtures")]
pub fn shot_just_added(id: &str) {
    mark_added(id);
}

/// For the shots: nothing was added just now, so one scene's add doesn't carry into the next.
#[cfg(feature = "fixtures")]
pub fn shot_clear_just_added() {
    JUST_ADDED.with_borrow_mut(Vec::clear);
}

fn added(what: Added) {
    if let Some(f) = ON_ADDED.with_borrow(Clone::clone) {
        f(what);
    }
}

/// Adds show as pages of `nav` under `step` ("5 of 5") from now on, or as dialogs with `None`.
pub fn show_in(pages: Option<(adw::NavigationView, String)>) {
    IN_PAGES.replace(pages);
}

/// The form every add takes: a header bar with Cancel and the one action over a scrolling
/// column, or inside first run its page. Returns where it shows, its action and the column.
pub fn form(
    ctx: &Rc<Ctx>,
    title: &str,
    action: &str,
    name: &str,
) -> (Host, gtk::Button, gtk::Box) {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 14);
    let primary = gtk::Button::with_label(action);
    primary.add_css_class("suggested-action");
    primary.set_sensitive(false);
    if let Some((nav, step)) = IN_PAGES.with_borrow(Clone::clone) {
        primary.add_css_class("pill");
        primary.set_halign(gtk::Align::Center);
        let page = crate::pages::first_launch::form_page(name, &step, title, &column, &primary);
        return (Host::Page { page, nav }, primary, column);
    }
    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);
    header.set_show_start_title_buttons(false);
    let cancel = gtk::Button::with_label(&t::action_cancel());
    header.pack_start(&cancel);
    header.pack_end(&primary);
    column.set_margin_top(12);
    column.set_margin_bottom(24);
    column.set_margin_start(18);
    column.set_margin_end(18);
    if ctx.size() != Size::Narrow {
        column.set_size_request(480, -1);
    }
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(640)
        .child(&column)
        .build();
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&scroll));
    let dialog = adw::Dialog::builder()
        .name(name)
        .title(title)
        .follows_content_size(true)
        .child(&view)
        .build();
    cancel.connect_clicked({
        let dialog = dialog.downgrade();
        move |_| {
            if let Some(dialog) = dialog.upgrade() {
                dialog.close();
            }
        }
    });
    (Host::Dialog(dialog), primary, column)
}

/// Close the add `name` if one is up, so a second add replaces the first.
fn replace(ctx: &Rc<Ctx>, name: &str) {
    if let Some(dialog) = ctx.window.visible_dialog()
        && dialog.widget_name() == name
    {
        dialog.force_close();
    }
    if let Some((nav, _)) = IN_PAGES.with_borrow(Clone::clone)
        && let Some(page) = nav.visible_page()
        && page.tag().as_deref() == Some(name)
    {
        Host::Page { page, nav }.force_close();
    }
}

/// Hear the engine while the add is up.
fn listen_while(ctx: &Rc<Ctx>, dialog: &Host, heard: impl Fn(&Event) + 'static) {
    let id = ctx.hub.listen(heard);
    let hub = Rc::downgrade(&ctx.hub);
    dialog.on_closed(move || {
        if let Some(hub) = hub.upgrade() {
            hub.forget(id);
        }
    });
}

/// A server's kind in words: Komga, Kavita, OPDS.
pub fn kind_words(kind: BookServerKind) -> String {
    match kind {
        BookServerKind::Komga => "Komga",   // i18n-skip
        BookServerKind::Kavita => "Kavita", // i18n-skip
        BookServerKind::Opds => "OPDS",     // i18n-skip
    }
    .to_owned()
}

/// Where this computer keeps a server's key or password.
pub fn key_store() -> String {
    t::secure_store_linux()
}

// ---------------------------------------------------------------------------------------------
// Addresses

/// A Kavita OPDS link: where it is, its host, and its key's last four characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KavitaLink {
    pub root: String,
    pub host: String,
    pub last4: String,
}

/// The Kavita OPDS link in `text`, as the engine reads one: https when no scheme is given, the
/// path perhaps under a proxy's prefix, `api/opds` in any case, then a key of 8 to 128 letters,
/// digits, `-` or `_`, and no user in the address.
pub fn kavita(text: &str) -> Option<KavitaLink> {
    let typed = text.trim();
    if typed.is_empty() || typed.chars().any(char::is_whitespace) {
        return None;
    }
    let (scheme, rest) = if let Some(rest) = typed.strip_prefix("https://") {
        ("https", rest)
    } else if let Some(rest) = typed.strip_prefix("http://") {
        ("http", rest)
    } else {
        ("https", typed)
    };
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    if authority.contains('@') || authority.is_empty() {
        return None;
    }
    let path = path.split(['?', '#']).next().unwrap_or("");
    let segments: Vec<&str> = path.split('/').collect();
    let at = segments.windows(2).position(|pair| {
        pair[0].eq_ignore_ascii_case("api") && pair[1].eq_ignore_ascii_case("opds")
    })?;
    let key = segments.get(at + 2)?;
    let good = (8..=128).contains(&key.len())
        && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !good {
        return None;
    }
    let host = authority.rsplit_once(':').map_or(authority, |(h, _)| h).to_lowercase();
    let mut root = format!("{scheme}://{}", authority.to_lowercase()); // i18n-skip
    for part in segments[..at].iter().filter(|p| !p.is_empty()) {
        root.push('/');
        root.push_str(part);
    }
    Some(KavitaLink { root, host, last4: key[key.len() - 4..].to_owned() })
}

/// Whether `text` could name a server yet: a dot, a port or localhost in it.
pub fn looks_like_address(text: &str) -> bool {
    let text = text.trim();
    let bare =
        text.strip_prefix("https://").or_else(|| text.strip_prefix("http://")).unwrap_or(text);
    bare.chars().count() >= 3
        && !bare.chars().any(char::is_whitespace)
        && (bare.contains('.') || bare.contains(':') || bare.starts_with("localhost"))
}

/// The host in an address, for the lines that name it.
pub fn host_of(address: &str) -> String {
    let text = address.trim();
    let bare = text.split_once("://").map_or(text, |(_, rest)| rest);
    let authority = bare.split(['/', '?', '#']).next().unwrap_or(bare);
    let authority = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    authority
        .rsplit_once(':')
        .filter(|(_, p)| p.chars().all(|c| c.is_ascii_digit()))
        .map_or(authority, |(h, _)| h)
        .to_owned()
}

// ---------------------------------------------------------------------------------------------
// Add a server

/// Where the server form stands.
#[derive(Debug, Clone, PartialEq)]
enum Step {
    /// Nothing asked of this address yet, or asked under 600 ms ago.
    Typing,
    Looking,
    /// It wants a username and password; `failed` once one was refused.
    SignIn {
        kind: BookServerKind,
        host: String,
        failed: bool,
    },
    Found(ServerFound),
    /// It let us in and shows no series.
    Empty(ServerFound),
    /// Unreachable, a refused key, or not a book server.
    Failed(NothingCameIn),
}

struct Server {
    ctx: Rc<Ctx>,
    dialog: Host,
    primary: gtk::Button,
    address: adw::EntryRow,
    chip: gtk::Label,
    /// A pasted Kavita link: what the field shows with its key masked, and the link itself.
    link: RefCell<Option<(String, String)>>,
    /// The field is being set to its masked form, so the change is not an edit.
    masking: Cell<bool>,
    hint: gtk::Label,
    kept: gtk::Revealer,
    sign_in: gtk::Revealer,
    sign_in_note: gtk::Box,
    user: adw::EntryRow,
    password: adw::PasswordEntryRow,
    result: Rc<Swap>,
    wait: Wait,
    /// The address asked, and whether with a password.
    asked: RefCell<Option<(String, bool)>>,
    step: RefCell<Step>,
    /// The step the result area shows, so a repeat is not drawn again.
    drawn: RefCell<Option<Step>>,
    ticked: RefCell<HashSet<String>>,
    progress: Cell<bool>,
    adding: Cell<bool>,
    edits: Cell<u64>,
}

const SERVER: &str = "add-book-server";

/// Add a book server: the address (or a Kavita OPDS link), what the server answers, and Add
/// server once something was found. `address` starts the field, as Edit does.
pub fn add_server(ctx: &Rc<Ctx>, address: Option<&str>) {
    replace(ctx, SERVER);
    let (dialog, primary, column) =
        form(ctx, &t::book_add_server_title(), &t::book_add_server_button(), SERVER);
    let fields = widgets::boxed_list();
    let entry = adw::EntryRow::builder().title(t::book_server_address()).build();
    entry.set_input_purpose(gtk::InputPurpose::Url);
    let chip = gtk::Label::new(None);
    chip.add_css_class("key-chip");
    chip.set_valign(gtk::Align::Center);
    chip.set_visible(false);
    entry.add_suffix(&chip);
    fields.append(&entry);
    column.append(&fields);
    // The hint, and under it the line about the key once a Kavita link is in.
    let notes = gtk::Box::new(gtk::Orientation::Vertical, 2);
    notes.set_margin_top(-6);
    let hint = widgets::haze(&t::book_server_address_hint(), "caption");
    notes.append(&hint);
    let kept = gtk::Revealer::builder()
        .transition_type(gtk::RevealerTransitionType::SlideDown)
        .transition_duration(if motion::reduced() { 0 } else { 300 })
        .child(&widgets::haze(&t::book_server_key_kept(&key_store()), "caption"))
        .build();
    notes.append(&kept);
    // The key's line shows only with the hint, so the box goes when the hint does.
    hint.bind_property("visible", &notes, "visible").sync_create().build();
    column.append(&notes);
    let sign_in_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let sign_in_note = nothing();
    sign_in_box.append(&sign_in_note);
    let credentials = widgets::boxed_list();
    let user = adw::EntryRow::builder().title(t::book_server_username()).build();
    let password = adw::PasswordEntryRow::builder().title(t::book_server_password()).build();
    credentials.append(&user);
    credentials.append(&password);
    sign_in_box.append(&credentials);
    let sign_in = gtk::Revealer::builder()
        .transition_type(gtk::RevealerTransitionType::SlideDown)
        .transition_duration(if motion::reduced() { 0 } else { 300 })
        .child(&sign_in_box)
        .build();
    column.append(&sign_in);
    let result = Swap::new();
    column.append(&result.stack);
    result.show(&nothing());

    let state = Rc::new(Server {
        ctx: Rc::clone(ctx),
        dialog: dialog.clone(),
        primary: primary.clone(),
        address: entry.clone(),
        chip,
        link: RefCell::default(),
        masking: Cell::new(false),
        hint,
        kept,
        sign_in,
        sign_in_note,
        user: user.clone(),
        password: password.clone(),
        result,
        wait: Wait::default(),
        asked: RefCell::default(),
        step: RefCell::new(Step::Typing),
        drawn: RefCell::default(),
        ticked: RefCell::default(),
        progress: Cell::new(true),
        adding: Cell::new(false),
        edits: Cell::new(0),
    });
    entry.connect_changed({
        let weak = Rc::downgrade(&state);
        move |_| {
            if let Some(state) = weak.upgrade() {
                state.edited();
            }
        }
    });
    entry.connect_entry_activated({
        let weak = Rc::downgrade(&state);
        move |_| {
            if let Some(state) = weak.upgrade() {
                state.returned();
            }
        }
    });
    for row in [user.upcast_ref::<adw::EntryRow>(), password.upcast_ref::<adw::EntryRow>()] {
        row.connect_changed({
            let weak = Rc::downgrade(&state);
            move |_| {
                if let Some(state) = weak.upgrade() {
                    state.refresh_primary();
                }
            }
        });
        row.connect_entry_activated({
            let weak = Rc::downgrade(&state);
            move |_| {
                if let Some(state) = weak.upgrade() {
                    state.press();
                }
            }
        });
    }
    primary.connect_clicked({
        let weak = Rc::downgrade(&state);
        move |_| {
            if let Some(state) = weak.upgrade() {
                state.press();
            }
        }
    });
    listen_while(ctx, &dialog, {
        let weak = Rc::downgrade(&state);
        move |event| {
            if let (Some(state), Event::CameIn(came)) = (weak.upgrade(), event) {
                state.heard(came);
            }
        }
    });
    widgets::keep(&dialog.widget(), Rc::clone(&state));
    if let Some(address) = address {
        entry.set_text(address);
    }
    dialog.present(ctx);
    entry.grab_focus();
    shown(&state);
}

/// The server form open on screen, for the shots that set its states.
#[cfg(feature = "fixtures")]
pub fn shot_server(ctx: &Rc<Ctx>, address: &str, came: Option<CameIn>, looking: bool) {
    add_server(ctx, Some(address));
    let Some(state) = with_server(Rc::clone) else { return };
    state.edits.set(state.edits.get() + 1);
    state.asked.replace(Some((address.trim().to_owned(), false)));
    if looking {
        state.step.replace(Step::Looking);
        state.draw();
    }
    if let Some(came) = came {
        state.answered(&came);
    }
}

/// The server form after a username and password were tried and refused, for the shots.
#[cfg(feature = "fixtures")]
pub fn shot_server_refused(
    ctx: &Rc<Ctx>,
    address: &str,
    (user, password): (&str, &str),
    came: CameIn,
) {
    shot_server(ctx, address, None, false);
    let Some(state) = with_server(Rc::clone) else { return };
    state.user.set_text(user);
    state.password.set_text(password);
    state.asked.replace(Some((address.trim().to_owned(), true)));
    state.answered(&came);
}

thread_local! {
    static SHOWN: RefCell<Option<std::rc::Weak<Server>>> = const { RefCell::new(None) };
}

fn shown(state: &Rc<Server>) {
    SHOWN.with(|s| s.replace(Some(Rc::downgrade(state))));
}

#[cfg(feature = "fixtures")]
fn with_server<R>(f: impl FnOnce(&Rc<Server>) -> R) -> Option<R> {
    SHOWN.with(|s| s.borrow().as_ref().and_then(std::rc::Weak::upgrade).map(|s| f(&s)))
}

impl Server {
    /// The address as typed or pasted: a Kavita link whole, though the field masks its key.
    fn text(&self) -> String {
        let shown = self.address.text().trim().to_owned();
        match self.link.borrow().as_ref() {
            Some((masked, full)) if *masked == shown => full.clone(),
            _ => shown,
        }
    }

    /// The address changed: an answer for another address no longer counts, and the new one is
    /// looked at once typing rests (a pasted Kavita link at once).
    fn edited(self: &Rc<Self>) {
        if self.masking.get() {
            return;
        }
        let shown = self.address.text().trim().to_owned();
        let masked = self.link.borrow().as_ref().is_some_and(|(masked, _)| *masked == shown);
        if !masked {
            self.link.replace(None);
            if let Some(link) = kavita(&shown) {
                // The field keeps the link's start; its key stays out of sight, in the chip.
                let start = format!("{}/api/opds/", link.root); // i18n-skip
                self.link.replace(Some((start.clone(), shown)));
                self.masking.set(true);
                self.address.set_text(&start);
                self.address.set_position(-1);
                self.masking.set(false);
            }
        }
        let edit = self.edits.get() + 1;
        self.edits.set(edit);
        let text = self.text();
        let link = kavita(&text);
        match &link {
            Some(link) => {
                self.chip.set_label(&t::book_server_key(&link.last4));
                self.chip.set_visible(true);
                self.hint.set_label(&t::book_server_kavita_key());
            }
            None => {
                self.chip.set_visible(false);
                self.hint.set_label(&t::book_server_address_hint());
            }
        }
        let same = self.asked.borrow().as_ref().is_some_and(|(asked, _)| *asked == text);
        if !same {
            self.wait.cancel();
            self.asked.replace(None);
            self.step.replace(Step::Typing);
            self.draw();
        }
        if same || text.is_empty() || (link.is_none() && !looks_like_address(&text)) {
            return;
        }
        let rest = if link.is_some() { 0 } else { 800 };
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(Duration::from_millis(rest), move || {
            if let Some(state) = weak.upgrade()
                && state.edits.get() == edit
                && state.asked.borrow().is_none()
            {
                state.probe(false);
            }
        });
    }

    fn probe(self: &Rc<Self>, with_password: bool) {
        let address = self.text();
        if address.is_empty() {
            return;
        }
        self.asked.replace(Some((address.clone(), with_password)));
        let (user, password) = if with_password && kavita(&address).is_none() {
            (
                Some(self.user.text().trim().to_owned()),
                Some(Secret(self.password.text().to_string())),
            )
        } else {
            (None, None)
        };
        let weak = Rc::downgrade(self);
        self.wait.start(move || {
            if let Some(state) = weak.upgrade() {
                // A sign-in being tried keeps its fields; the line takes the result's place.
                state.step.replace(Step::Looking);
                state.draw();
            }
        });
        self.refresh_primary();
        send(&self.ctx.hub, BookSourceCommand::Probe { address, user, password });
    }

    fn heard(self: &Rc<Self>, came: &CameIn) {
        if self.adding.get() {
            if let CameIn::ServerAdded(server) = came {
                let toast = server.series.map(|n| t::source_added(&server.host, i64::from(n)));
                let kind = kind_words(server.kind);
                mark_added(&server.id);
                added(Added {
                    icon: "network-server-symbolic",
                    title: server.host.clone(),
                    line: match server.series {
                        Some(series) if !server.empty => t::source_row_server(
                            &kind,
                            i64::from(series),
                            i64::from(server.libraries.max(1)),
                        ),
                        _ => t::source_row_server_empty(&kind),
                    },
                });
                self.dialog.close();
                if let Some(toast) = toast {
                    self.ctx.toast(&toast);
                }
            }
            return;
        }
        if self.asked.borrow().is_none() || !is_server_answer(came) {
            return;
        }
        let weak = Rc::downgrade(self);
        let came = came.clone();
        self.wait.end(move || {
            if let Some(state) = weak.upgrade() {
                state.answered(&came);
            }
        });
    }

    fn answered(&self, came: &CameIn) {
        let with_password = self.asked.borrow().as_ref().is_some_and(|(_, p)| *p);
        let step = match came {
            CameIn::Found(found) => Step::Found(found.clone()),
            CameIn::Empty(NothingCameIn::EmptyCatalogue { found }) => {
                Step::Empty(found.clone())
            }
            CameIn::Empty(NothingCameIn::WrongPassword { host, kind }) => {
                Step::SignIn { kind: *kind, host: host.clone(), failed: with_password }
            }
            CameIn::Empty(why) => Step::Failed(why.clone()),
            _ => return,
        };
        if let Step::Found(found) = &step {
            self.ticked.replace(found.libraries.iter().map(|l| l.id.clone()).collect());
        }
        self.step.replace(step);
        self.draw();
    }

    fn press(self: &Rc<Self>) {
        if !self.primary.is_sensitive() {
            return;
        }
        let step = self.step.borrow().clone();
        match step {
            Step::Found(found) | Step::Empty(found) => {
                let all: Vec<String> = found.libraries.iter().map(|l| l.id.clone()).collect();
                let ticked = self.ticked.borrow();
                // Every library ticked asks for all of them, those made later too.
                let libraries = if all.iter().all(|id| ticked.contains(id)) {
                    Vec::new()
                } else {
                    all.into_iter().filter(|id| ticked.contains(id)).collect()
                };
                self.adding.set(true);
                self.primary.set_sensitive(false);
                send(
                    &self.ctx.hub,
                    BookSourceCommand::AddServer {
                        pending: found.pending.clone(),
                        libraries,
                        report_progress: self.progress.get()
                            && found.kind != BookServerKind::Opds,
                    },
                );
            }
            Step::SignIn { .. } => self.probe(true),
            Step::Typing | Step::Failed(_) => self.probe(false),
            Step::Looking => {}
        }
    }

    /// Return in the address: while it is being typed, or after it failed, it looks at once
    /// instead of waiting for typing to rest. Add server stays off until the server answers.
    fn returned(self: &Rc<Self>) {
        let text = self.text();
        let parses = kavita(&text).is_some() || looks_like_address(&text);
        let fresh = matches!(*self.step.borrow(), Step::Typing | Step::Failed(_));
        if fresh {
            if parses && !self.asking() && !self.adding.get() {
                self.edits.set(self.edits.get() + 1);
                self.probe(false);
            }
        } else {
            self.press();
        }
    }

    fn asking(&self) -> bool {
        self.asked.borrow().is_some() && *self.step.borrow() == Step::Looking
            || (self.asked.borrow().is_some() && !self.answer_in())
    }

    /// Whether the address asked has its answer in.
    fn answer_in(&self) -> bool {
        !matches!(*self.step.borrow(), Step::Typing | Step::Looking)
    }

    fn refresh_primary(&self) {
        let text = self.text();
        let parses = kavita(&text).is_some() || looks_like_address(&text);
        let step = self.step.borrow().clone();
        let (label, on) = match &step {
            Step::Found(found) => (
                t::book_add_server_button(),
                found.libraries.is_empty() || !self.ticked.borrow().is_empty(),
            ),
            Step::Empty(_) => (t::book_add_server_button(), true),
            Step::SignIn { .. } => (
                t::book_add_server_button(),
                !self.user.text().trim().is_empty() && !self.password.text().is_empty(),
            ),
            // The likely fix is the address, so Try again sits under the error and Add server
            // waits for an answer, as it does while typing and looking.
            Step::Failed(_) | Step::Typing | Step::Looking => {
                (t::book_add_server_button(), false)
            }
        };
        self.primary.set_label(&label);
        self.primary.set_sensitive(parses && on && !self.adding.get() && !self.asking());
    }

    fn draw(&self) {
        let step = self.step.borrow().clone();
        self.refresh_primary();
        let link = kavita(&self.text());
        self.kept
            .set_reveal_child(link.is_some() && matches!(step, Step::Typing | Step::Looking));
        self.hint.set_visible(matches!(step, Step::Typing | Step::Looking));
        // The sign-in fields: shown while a server wants them, and kept while a password is tried.
        let wants = match &step {
            Step::SignIn { kind, .. } => Some(*kind),
            Step::Looking => match self.drawn.borrow().as_ref() {
                Some(Step::SignIn { kind, .. }) => Some(*kind),
                _ => None,
            },
            _ => None,
        };
        if let Some(kind) = wants.filter(|_| link.is_none()) {
            if self.sign_in_note.first_child().is_none() {
                self.sign_in_note.append(&note(
                    Mark::Found,
                    &t::sources_found_server(&kind_words(kind)),
                    None,
                ));
            }
            if !self.sign_in.reveals_child() {
                self.sign_in.set_reveal_child(true);
                self.user.grab_focus();
            }
        } else {
            self.sign_in.set_reveal_child(false);
            while let Some(child) = self.sign_in_note.first_child() {
                self.sign_in_note.remove(&child);
            }
        }
        if self.drawn.borrow().as_ref() == Some(&step) {
            return;
        }
        let shown: gtk::Widget = match &step {
            Step::Typing => nothing().upcast(),
            Step::Looking => waiting(&t::book_server_looking()).upcast(),
            Step::SignIn { failed: false, .. } => nothing().upcast(),
            Step::SignIn { kind, host, failed: true } => note(
                Mark::Failed,
                &t::book_server_err_password(host),
                Some(&t::book_server_err_password_line(&kind_words(*kind))),
            )
            .upcast(),
            Step::Found(found) => self.found(found).upcast(),
            Step::Empty(found) => {
                let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
                column.append(&note(
                    Mark::Info,
                    &t::book_server_empty(&found.host),
                    Some(&t::book_server_empty_line()),
                ));
                column.append(&widgets::haze(&t::book_server_empty_recheck(), ""));
                column.upcast()
            }
            Step::Failed(why @ NothingCameIn::Unreachable { .. }) => {
                let column = gtk::Box::new(gtk::Orientation::Vertical, 10);
                column.append(&failed_note(why));
                let again = gtk::Button::with_label(&t::action_try_again());
                again.add_css_class("pill");
                again.set_halign(gtk::Align::Start);
                // Under the error's words: past the glyph and the gap beside it.
                again.set_margin_start(28);
                let me = SHOWN.with(|s| s.borrow().clone());
                again.connect_clicked(move |_| {
                    if let Some(state) = me.as_ref().and_then(std::rc::Weak::upgrade) {
                        state.edits.set(state.edits.get() + 1);
                        state.probe(false);
                    }
                });
                column.append(&again);
                column.upcast()
            }
            Step::Failed(why) => failed_note(why).upcast(),
        };
        self.drawn.replace(Some(step));
        self.result.show(&shown);
    }

    /// Found: the series in how many libraries, each library to tick, and Send progress.
    fn found(&self, found: &ServerFound) -> gtk::Box {
        let column = gtk::Box::new(gtk::Orientation::Vertical, 14);
        let series = found
            .series
            .unwrap_or_else(|| found.libraries.iter().filter_map(|l| l.series).sum::<u32>());
        column.append(&note(
            Mark::Found,
            &t::book_server_found_title(i64::from(series), found.libraries.len() as i64),
            Some(&t::book_server_found_line(&kind_words(found.kind), &found.host)),
        ));
        if !found.libraries.is_empty() {
            let list = widgets::boxed_list();
            for library in &found.libraries {
                let check = gtk::CheckButton::new();
                check.set_active(self.ticked.borrow().contains(&library.id));
                let row = adw::ActionRow::builder()
                    .title(glib::markup_escape_text(&library.name))
                    .subtitle(t::book_server_library_line(i64::from(
                        library.series.unwrap_or(0),
                    )))
                    .activatable_widget(&check)
                    .build();
                row.add_prefix(&check);
                let (id, me) = (library.id.clone(), SHOWN.with(|s| s.borrow().clone()));
                check.connect_toggled(move |check| {
                    if let Some(state) = me.as_ref().and_then(std::rc::Weak::upgrade) {
                        if check.is_active() {
                            state.ticked.borrow_mut().insert(id.clone());
                        } else {
                            state.ticked.borrow_mut().remove(&id);
                        }
                        state.refresh_primary();
                    }
                });
                list.append(&row);
            }
            column.append(&list);
        }
        if found.kind != BookServerKind::Opds {
            let list = widgets::boxed_list();
            let progress = adw::SwitchRow::builder()
                .title(t::book_server_sync_progress())
                .subtitle(glib::markup_escape_text(&t::book_server_sync_progress_line(
                    &found.host,
                )))
                .active(self.progress.get())
                .build();
            let me = SHOWN.with(|s| s.borrow().clone());
            progress.connect_active_notify(move |row| {
                if let Some(state) = me.as_ref().and_then(std::rc::Weak::upgrade) {
                    state.progress.set(row.is_active());
                }
            });
            list.append(&progress);
            column.append(&list);
        }
        column
    }
}

fn is_server_answer(came: &CameIn) -> bool {
    matches!(
        came,
        CameIn::Found(_)
            | CameIn::Empty(
                NothingCameIn::Unreachable { .. }
                    | NothingCameIn::WrongPassword { .. }
                    | NothingCameIn::WrongKey { .. }
                    | NothingCameIn::NotABookServer { .. }
                    | NothingCameIn::EmptyCatalogue { .. }
            )
    )
}

fn failed_note(why: &NothingCameIn) -> gtk::Box {
    match why {
        NothingCameIn::Unreachable { host } => note(
            Mark::Failed,
            &t::book_server_err_unreachable(host),
            Some(&t::book_server_err_unreachable_line()),
        ),
        NothingCameIn::WrongKey { host } => note(
            Mark::Failed,
            &t::book_server_err_key(host),
            Some(&t::book_server_err_key_line()),
        ),
        _ => note(
            Mark::Failed,
            &t::book_sources_not_a_book_server(),
            Some(&t::book_server_not_books_line()),
        ),
    }
}

// ---------------------------------------------------------------------------------------------
// Add a folder

const FOLDER: &str = "add-book-folder";

/// Choose a folder, then say what it holds: Add folder keeps it, Cancel takes it back out.
pub fn add_folder(ctx: &Rc<Ctx>) {
    let dialog = gtk::FileDialog::new();
    dialog.set_title(&t::book_add_folder());
    let ctx_weak = Rc::downgrade(ctx);
    dialog.select_folder(Some(&ctx.window), gio::Cancellable::NONE, move |result| {
        let (Ok(file), Some(ctx)) = (result, ctx_weak.upgrade()) else { return };
        let path = file.path().map(|p| p.to_string_lossy().into_owned());
        let name = file.basename().map(|p| p.to_string_lossy().into_owned());
        send(
            &ctx.hub,
            BookSourceCommand::AddFolder {
                folder: FolderRef {
                    path: path.clone(),
                    tree_uri: None,
                    bookmark: None,
                    name: name.clone(),
                },
            },
        );
        folder_dialog(&ctx, name.unwrap_or_default(), path, None);
    });
}

struct Folder {
    ctx: Rc<Ctx>,
    dialog: Host,
    primary: gtk::Button,
    name: String,
    result: Rc<Swap>,
    wait: Wait,
    report: RefCell<Option<(FolderReport, bool)>>,
    kept: Cell<bool>,
    scanning: RefCell<Option<gtk::Label>>,
}

/// The folder's dialog: the folder with Change, the scan, then what it found. `answer` is set
/// for the shots.
fn folder_dialog(ctx: &Rc<Ctx>, name: String, path: Option<String>, answer: Option<CameIn>) {
    replace(ctx, FOLDER);
    let (dialog, primary, column) =
        form(ctx, &t::sources_add_folder_button(), &t::book_folder_add(), FOLDER);
    let card = widgets::boxed_list();
    let row = adw::ActionRow::builder()
        .title(glib::markup_escape_text(&name))
        .subtitle(glib::markup_escape_text(path.as_deref().unwrap_or("")))
        .build();
    row.add_prefix(&tile("folder-symbolic"));
    let change = gtk::Button::with_label(&t::book_folder_change());
    change.set_valign(gtk::Align::Center);
    row.add_suffix(&change);
    card.append(&row);
    column.append(&card);
    let result = Swap::new();
    column.append(&result.stack);
    result.show(&nothing());
    let state = Rc::new(Folder {
        ctx: Rc::clone(ctx),
        dialog: dialog.clone(),
        primary: primary.clone(),
        name,
        result,
        wait: Wait::default(),
        report: RefCell::default(),
        kept: Cell::new(false),
        scanning: RefCell::default(),
    });
    change.connect_clicked({
        let weak = Rc::downgrade(&state);
        move |_| {
            if let Some(state) = weak.upgrade() {
                state.another();
            }
        }
    });
    primary.connect_clicked({
        let weak = Rc::downgrade(&state);
        move |_| {
            let Some(state) = weak.upgrade() else { return };
            match state.report.borrow().clone() {
                Some((report, true)) => {
                    state.kept.set(true);
                    let skipped: u32 =
                        report.unsupported.iter().chain(&report.locked).map(|f| f.count).sum();
                    added(Added {
                        icon: "folder-symbolic",
                        title: report.name.clone(),
                        line: t::source_row_folder(
                            report.path.as_deref().unwrap_or(&report.name),
                            i64::from(report.series),
                            i64::from(skipped),
                        ),
                    });
                    state.dialog.close();
                }
                Some(_) => state.another(),
                None => {}
            }
        }
    });
    dialog.on_closed({
        let weak = Rc::downgrade(&state);
        move || {
            if let Some(state) = weak.upgrade()
                && !state.kept.get()
            {
                state.take_back();
            }
        }
    });
    listen_while(ctx, &dialog, {
        let weak = Rc::downgrade(&state);
        move |event| {
            let Some(state) = weak.upgrade() else { return };
            match event {
                Event::CameIn(came) => state.heard(came),
                Event::BookSources(BookSourceEvent::View(view)) => {
                    let found = view.folders.iter().find(|f| f.name == state.name).and_then(
                        |f| match f.scan {
                            kaeri_model::book_sources::BookFolderScan::Scanning { found } => {
                                Some(found)
                            }
                            _ => None,
                        },
                    );
                    if let (Some(found), Some(label)) =
                        (found, state.scanning.borrow().as_ref())
                    {
                        label.set_label(&t::book_sources_scanning(i64::from(found)));
                    }
                }
                _ => {}
            }
        }
    });
    widgets::keep(&dialog.widget(), Rc::clone(&state));
    dialog.present(ctx);
    let weak = Rc::downgrade(&state);
    state.wait.start(move || {
        if let Some(state) = weak.upgrade() {
            let line = waiting(&t::book_sources_scanning(0));
            if let Some(label) = line.last_child().and_downcast::<gtk::Label>() {
                state.scanning.replace(Some(label));
            }
            state.result.show(&line);
        }
    });
    if let Some(answer) = answer {
        state.heard(&answer);
    }
}

/// The folder dialog with its answer in, for the shots.
#[cfg(feature = "fixtures")]
pub fn shot_folder(ctx: &Rc<Ctx>, came: CameIn) {
    let (name, path) = match &came {
        CameIn::Folder(r)
        | CameIn::Empty(
            NothingCameIn::UnsupportedFiles { folder: r }
            | NothingCameIn::EmptyFolder { folder: r },
        ) => (r.name.clone(), r.path.clone()),
        _ => return,
    };
    folder_dialog(ctx, name, path, Some(came));
}

impl Folder {
    fn heard(self: &Rc<Self>, came: &CameIn) {
        let report = match came {
            CameIn::Folder(report) => (report.clone(), report.series > 0),
            CameIn::Empty(
                NothingCameIn::UnsupportedFiles { folder }
                | NothingCameIn::EmptyFolder { folder },
            ) if folder.name == self.name => (folder.clone(), false),
            _ => return,
        };
        if report.0.name != self.name {
            return;
        }
        let weak = Rc::downgrade(self);
        self.wait.end(move || {
            if let Some(state) = weak.upgrade() {
                state.show(report);
            }
        });
    }

    fn show(&self, (report, found): (FolderReport, bool)) {
        let column = gtk::Box::new(gtk::Orientation::Vertical, 14);
        if found {
            column.append(&note(
                Mark::Found,
                &t::book_folder_found(i64::from(report.series), i64::from(report.files)),
                Some(&t::book_folder_found_line(
                    report.path.as_deref().unwrap_or(&report.name),
                    i64::from(report.matched),
                )),
            ));
        } else {
            column.append(&note(
                Mark::Info,
                &t::book_folder_empty(&report.name),
                Some(&t::book_folder_empty_line(i64::from(report.files))),
            ));
        }
        if report.skipped() > 0 {
            column.append(&unsupported(&report));
        }
        column.append(&widgets::haze(&t::book_folder_opens(), "caption"));
        self.primary.set_label(&if found {
            t::book_folder_add()
        } else {
            t::book_folder_choose_other()
        });
        self.primary.set_sensitive(true);
        self.result.show(&column);
        self.report.replace(Some((report, found)));
    }

    /// Take the folder back out: nothing was meant to be added yet.
    fn take_back(&self) {
        if let Some((report, _)) = self.report.borrow().as_ref() {
            send(&self.ctx.hub, BookSourceCommand::RemoveFolder { id: report.id.clone() });
        }
    }

    fn another(&self) {
        self.take_back();
        self.kept.set(true);
        self.dialog.close();
        add_folder(&self.ctx);
    }
}

/// The files it can't open, by format, with the list behind Show files.
fn unsupported(report: &FolderReport) -> gtk::ListBox {
    let list = widgets::boxed_list();
    let mut kinds: Vec<String> =
        report.unsupported.iter().map(|f| format!("{} {}", f.count, f.format)).collect(); // i18n-skip: a count and a format
    let locked: u32 = report.locked.iter().map(|f| f.count).sum();
    if locked > 0 {
        kinds.push(t::book_folder_drm(i64::from(locked)));
    }
    let title = t::book_folder_unsupported(i64::from(report.skipped()));
    let line = t::book_folder_unsupported_line(&kinds.join(", ")); // i18n-skip
    if report.listed.is_empty() {
        list.append(
            &adw::ActionRow::builder()
                .title(glib::markup_escape_text(&title))
                .subtitle(glib::markup_escape_text(&line))
                .build(),
        );
        return list;
    }
    let row = adw::ExpanderRow::builder()
        .title(glib::markup_escape_text(&title))
        .subtitle(glib::markup_escape_text(&line))
        .build();
    let show = gtk::Label::new(Some(&t::book_folder_show_files()));
    show.add_css_class("dim-label");
    row.add_suffix(&show);
    for file in &report.listed {
        let file_row = adw::ActionRow::builder().title(glib::markup_escape_text(file)).build();
        file_row.add_css_class("property");
        row.add_row(&file_row);
    }
    list.append(&row);
    list
}

/// A row's leading tile: a symbolic glyph on a raised square.
pub fn tile(icon: &str) -> gtk::Box {
    let tile = gtk::Box::new(gtk::Orientation::Vertical, 0);
    tile.add_css_class("source-tile");
    tile.set_valign(gtk::Align::Center);
    let image = gtk::Image::from_icon_name(icon);
    image.set_vexpand(true);
    tile.append(&image);
    tile
}

// ---------------------------------------------------------------------------------------------
// Join a plugin store

const STORE: &str = "join-book-store";

/// Where the store form stands.
#[derive(Clone)]
enum StoreStep {
    /// The invite, and its store once checked.
    Invite,
    Joined(StoreReport),
    Installed(PluginsInstalled),
    OtherMedium(StoreReport),
    Empty(StoreReport),
    Unreachable(String),
}

struct StoreForm {
    ctx: Rc<Ctx>,
    dialog: Host,
    primary: gtk::Button,
    invite: adw::EntryRow,
    chip: gtk::Label,
    /// A pasted invite: what the field shows, its store's host, and the link itself.
    link: RefCell<Option<(String, String)>>,
    /// The field is being set to its masked form, so the change is not an edit.
    masking: Cell<bool>,
    preview: Rc<Swap>,
    result: Rc<Swap>,
    wait: Wait,
    store: RefCell<Option<InvitedStore>>,
    step: RefCell<StoreStep>,
    /// What the store offered when it was joined, listed again once the plugins installed.
    joined: RefCell<Option<StoreReport>>,
    edits: Cell<u64>,
    joining: Cell<bool>,
    installing: Cell<bool>,
    /// The join's answer is in and waits to be drawn; an alert for it is said by that answer.
    answering: Cell<bool>,
}

/// Join a plugin store from Your sources: the invite, its store, Join, then what it offers for
/// reading and for watching, each to install.
pub fn join_store(ctx: &Rc<Ctx>, link: Option<&str>) {
    replace(ctx, STORE);
    let (dialog, primary, column) =
        form(ctx, &t::store_join_title(), &t::stores_join_button(), STORE);
    let fields = widgets::boxed_list();
    let entry = adw::EntryRow::builder().title(t::store_invite_field()).build();
    let chip = gtk::Label::new(None);
    chip.add_css_class("key-chip");
    chip.set_valign(gtk::Align::Center);
    chip.set_visible(false);
    entry.add_suffix(&chip);
    fields.append(&entry);
    column.append(&fields);
    let preview = Swap::new();
    column.append(&preview.stack);
    preview.show(&widgets::haze(&t::onboarding_books_plugin_line(), "caption"));
    let result = Swap::new();
    column.append(&result.stack);
    result.show(&nothing());
    let state = Rc::new(StoreForm {
        ctx: Rc::clone(ctx),
        dialog: dialog.clone(),
        primary: primary.clone(),
        invite: entry.clone(),
        chip,
        link: RefCell::default(),
        masking: Cell::new(false),
        preview,
        result,
        wait: Wait::default(),
        store: RefCell::default(),
        step: RefCell::new(StoreStep::Invite),
        joined: RefCell::default(),
        edits: Cell::new(0),
        joining: Cell::new(false),
        installing: Cell::new(false),
        answering: Cell::new(false),
    });
    entry.connect_changed({
        let weak = Rc::downgrade(&state);
        move |_| {
            if let Some(state) = weak.upgrade() {
                state.edited();
            }
        }
    });
    primary.connect_clicked({
        let weak = Rc::downgrade(&state);
        move |_| {
            if let Some(state) = weak.upgrade() {
                state.press();
            }
        }
    });
    listen_while(ctx, &dialog, {
        let weak = Rc::downgrade(&state);
        move |event| {
            let Some(state) = weak.upgrade() else { return };
            match event {
                Event::StorePreview(preview) => state.previewed(preview),
                Event::CameIn(came) => state.heard(came),
                Event::Stores(_) => state.draw_result(),
                _ => {}
            }
        }
    });
    // A join the engine could not make is said here, and Join can be pressed again.
    let claim = ctx.claim_alerts({
        let weak = Rc::downgrade(&state);
        move |_, why| {
            let Some(state) = weak.upgrade() else { return false };
            // An unreachable store comes as an answer and an alert: the answer says it here.
            if state.answering.get() {
                return true;
            }
            if !state.joining.replace(false) {
                return false;
            }
            state.wait.cancel();
            state.result.show(&note(Mark::Failed, why, None));
            state.primary.set_sensitive(state.store.borrow().is_some());
            true
        }
    });
    let ctx_weak = Rc::downgrade(ctx);
    dialog.on_closed(move || {
        if let Some(ctx) = ctx_weak.upgrade() {
            ctx.release_alerts(claim);
        }
    });
    widgets::keep(&dialog.widget(), Rc::clone(&state));
    if let Some(link) = link {
        entry.set_text(link);
    }
    STORE_SHOWN.with(|s| s.replace(Some(Rc::downgrade(&state))));
    dialog.present(ctx);
    entry.grab_focus();
}

thread_local! {
    static STORE_SHOWN: RefCell<Option<std::rc::Weak<StoreForm>>> = const { RefCell::new(None) };
}

/// The store form with its answer in, for the shots.
#[cfg(feature = "fixtures")]
pub fn shot_store(ctx: &Rc<Ctx>, link: &str, came: Option<CameIn>) {
    join_store(ctx, None);
    let Some(state) =
        STORE_SHOWN.with(|s| s.borrow().as_ref().and_then(std::rc::Weak::upgrade))
    else {
        return;
    };
    state.edits.set(state.edits.get() + 1);
    state.invite.set_text(link);
    state.edits.set(state.edits.get() + 1);
    if let Some(came) = came {
        state.joining.set(true);
        state.answered(&came);
    }
}

/// The open store form answered again, for the shots that step it on.
#[cfg(feature = "fixtures")]
pub fn shot_store_answer(came: &CameIn) {
    let Some(state) =
        STORE_SHOWN.with(|s| s.borrow().as_ref().and_then(std::rc::Weak::upgrade))
    else {
        return;
    };
    state.answered(came);
}

/// The open store form's invite previewed, for the shots.
#[cfg(feature = "fixtures")]
pub fn shot_store_preview(preview: &InvitePreview) {
    let Some(state) =
        STORE_SHOWN.with(|s| s.borrow().as_ref().and_then(std::rc::Weak::upgrade))
    else {
        return;
    };
    state.previewed(preview);
}

/// How many plugins a joined store offers, those for a medium this device doesn't show too.
fn offered(report: &StoreReport) -> i64 {
    (report.for_reading.len() + report.for_watching.len()) as i64
        + i64::from(report.other_medium)
}

impl StoreForm {
    /// The invite as pasted, though the field shows only its store's host.
    fn invite_link(&self) -> Option<String> {
        let shown = self.invite.text().trim().to_owned();
        let full = match self.link.borrow().as_ref() {
            Some((host, full)) if *host == shown => full.clone(),
            _ => shown,
        };
        crate::pages::join::invite_in(&full)
    }

    fn edited(self: &Rc<Self>) {
        if self.masking.get() {
            return;
        }
        // A new invite after an unreachable store starts over: its preview, then Join.
        if matches!(*self.step.borrow(), StoreStep::Unreachable(_)) {
            self.step.replace(StoreStep::Invite);
            self.result.show(&nothing());
            self.primary.set_label(&t::stores_join_button());
            self.preview.stack.set_visible(true);
        }
        let shown = self.invite.text().trim().to_owned();
        let kept = self.link.borrow().as_ref().filter(|(host, _)| *host == shown).cloned();
        let text = match kept {
            Some((_, full)) => full,
            None => {
                self.link.replace(None);
                // The field keeps the store's host; the invite's code stays out of sight, its
                // last characters in the chip.
                let host = crate::pages::join::invite_in(&shown).and_then(|link| {
                    let rest = link.split_once("://")?.1;
                    let host = rest.split(['/', '?', '#']).next()?.to_owned();
                    (link.contains('#') && !host.is_empty()).then_some(host)
                });
                match host {
                    Some(host) => {
                        self.link.replace(Some((host.clone(), shown.clone())));
                        self.masking.set(true);
                        self.invite.set_text(&host);
                        self.invite.set_position(-1);
                        self.masking.set(false);
                        shown
                    }
                    None => shown,
                }
            }
        };
        let edit = self.edits.get() + 1;
        self.edits.set(edit);
        self.store.replace(None);
        self.primary.set_sensitive(false);
        let link = crate::pages::join::invite_in(&text);
        match link.as_deref().and_then(|l| l.rsplit_once('#')).map(|(code, _)| code) {
            Some(code) => {
                let code = code.rsplit('/').next().unwrap_or(code);
                let last: String =
                    code.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
                self.chip.set_label(&t::linux_store_invite_chip(&last));
                self.chip.set_visible(true);
            }
            None => self.chip.set_visible(false),
        }
        let Some(link) = link else {
            if text.is_empty() {
                self.preview
                    .show(&widgets::haze(&t::onboarding_books_plugin_line(), "caption"));
            }
            return;
        };
        self.preview.show(&waiting(&t::stores_checking_link()));
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(Duration::from_millis(400), move || {
            if let Some(state) = weak.upgrade()
                && state.edits.get() == edit
            {
                state.ctx.hub.send(Command::PreviewStore { link: Secret(link) });
            }
        });
    }

    fn previewed(&self, preview: &InvitePreview) {
        if !matches!(*self.step.borrow(), StoreStep::Invite) {
            return;
        }
        match preview {
            InvitePreview::Failed { reason, contact } => {
                self.store.replace(None);
                self.primary.set_sensitive(false);
                let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
                column.append(&note(Mark::Failed, reason, None));
                if let Some(contact) = contact {
                    column.append(&widgets::haze(&t::store_invite_contact(contact), "caption"));
                }
                self.preview.show(&column);
            }
            InvitePreview::Store(store) => {
                let card = widgets::boxed_list();
                card.append(
                    &adw::ActionRow::builder()
                        .title(glib::markup_escape_text(&store.name))
                        .subtitle(glib::markup_escape_text(&crate::pages::stores::run_by(
                            &store.owners,
                            &store.host,
                        )))
                        .build(),
                );
                let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
                column.append(&card);
                column.append(&widgets::haze(
                    &if store.already_joined {
                        t::stores_already_joined_note()
                    } else {
                        t::onboarding_books_plugin_line()
                    },
                    "caption",
                ));
                self.preview.show(&column);
                self.primary.set_sensitive(!store.already_joined);
                self.store.replace(Some(store.clone()));
            }
        }
    }

    fn press(self: &Rc<Self>) {
        let step = self.step.borrow().clone();
        match step {
            StoreStep::Invite | StoreStep::Unreachable(_) => {
                // Try again after an unreachable store needs no preview: the invite is enough.
                let link = match self.store.borrow().as_ref() {
                    Some(store) => store.link.clone(),
                    None if matches!(step, StoreStep::Unreachable(_)) => {
                        let Some(link) = self.invite_link() else { return };
                        Secret(link)
                    }
                    None => return,
                };
                self.primary.set_sensitive(false);
                self.joining.set(true);
                let weak = Rc::downgrade(self);
                self.wait.start(move || {
                    if let Some(state) = weak.upgrade() {
                        state.result.show(&waiting(&t::linux_store_joining()));
                    }
                });
                self.ctx.hub.send(Command::JoinStore { link, label: None, name: None });
            }
            StoreStep::Joined(report) if report.install_all => {
                self.installing.set(true);
                self.primary.set_sensitive(false);
                let weak = Rc::downgrade(self);
                self.wait.start(move || {
                    if let Some(state) = weak.upgrade() {
                        state.primary.set_label(&t::linux_store_installing());
                    }
                });
                self.ctx
                    .hub
                    .send(Command::InstallAllForReading { store: report.store.clone() });
            }
            _ => self.dialog.close(),
        }
    }

    fn heard(self: &Rc<Self>, came: &CameIn) {
        let ours = matches!(
            came,
            CameIn::Store(_)
                | CameIn::Installed(_)
                | CameIn::Empty(
                    NothingCameIn::EmptyStore { .. }
                        | NothingCameIn::OtherMedium { .. }
                        | NothingCameIn::StoreUnreachable { .. }
                )
        );
        if !ours || !(self.joining.get() || self.installing.get()) {
            return;
        }
        self.answering.set(true);
        let weak = Rc::downgrade(self);
        let came = came.clone();
        self.wait.end(move || {
            if let Some(state) = weak.upgrade() {
                state.answered(&came);
            }
        });
    }

    fn answered(&self, came: &CameIn) {
        self.joining.set(false);
        self.installing.set(false);
        self.answering.set(false);
        if let CameIn::Store(report) = came {
            let count = offered(report);
            added(Added {
                icon: "system-software-install-symbolic",
                title: report.name.clone(),
                line: t::store_joined_line(
                    report.owner.as_deref().unwrap_or(&report.name),
                    count,
                ),
            });
        }
        let step = match came {
            CameIn::Store(report) => StoreStep::Joined(report.clone()),
            CameIn::Installed(installed) => StoreStep::Installed(installed.clone()),
            CameIn::Empty(NothingCameIn::EmptyStore { store }) => {
                StoreStep::Empty(store.clone())
            }
            CameIn::Empty(NothingCameIn::OtherMedium { store }) => {
                StoreStep::OtherMedium(store.clone())
            }
            CameIn::Empty(NothingCameIn::StoreUnreachable { host }) => {
                StoreStep::Unreachable(host.clone())
            }
            _ => return,
        };
        // Once joined, the invite has done its work: the field stays as a record of it, and its
        // preview leaves.
        let joined = !matches!(step, StoreStep::Unreachable(_));
        self.invite.set_editable(!joined);
        // The preview stays only as the store's card above an unreachable error; a preview still
        // being looked for gives way to the error.
        let card = !joined && self.store.borrow().is_some();
        if !card {
            self.edits.set(self.edits.get() + 1);
        }
        self.preview.stack.set_visible(card);
        if let StoreStep::Joined(report) = &step {
            self.joined.replace(Some(report.clone()));
        }
        self.step.replace(step);
        self.draw_result();
    }

    fn draw_result(&self) {
        let step = self.step.borrow().clone();
        let owner =
            |report: &StoreReport| report.owner.clone().unwrap_or_else(|| report.name.clone());
        let (shown, label, on): (gtk::Widget, String, bool) = match &step {
            StoreStep::Invite => return,
            StoreStep::Joined(report) => {
                let column = gtk::Box::new(gtk::Orientation::Vertical, 14);
                let count = offered(report);
                column.append(&note(
                    Mark::Found,
                    &t::store_joined_title(&report.name),
                    Some(&t::store_joined_line(&owner(report), count)),
                ));
                self.groups(&column, report);
                let label = if report.install_all {
                    t::store_install_all_reading()
                } else {
                    t::action_done()
                };
                (column.upcast(), label, true)
            }
            StoreStep::Installed(installed) => {
                let column = gtk::Box::new(gtk::Orientation::Vertical, 14);
                column.append(&note(
                    Mark::Found,
                    &t::store_installed(i64::from(installed.installed)),
                    None,
                ));
                if let Some(report) = self.joined.borrow().as_ref() {
                    self.groups(&column, report);
                }
                (column.upcast(), t::action_done(), true)
            }
            StoreStep::OtherMedium(report) => {
                let column = gtk::Box::new(gtk::Orientation::Vertical, 14);
                column.append(&note(
                    Mark::Info,
                    &t::store_other_medium(&report.name),
                    Some(&t::store_other_medium_line()),
                ));
                self.other_group(&column, report);
                let on = gtk::Button::with_label(&t::store_turn_on_watching());
                on.add_css_class("pill");
                on.set_halign(gtk::Align::Start);
                let hub = Rc::downgrade(&self.ctx.hub);
                on.connect_clicked(move |_| {
                    if let Some(hub) = hub.upgrade() {
                        hub.send(Command::Medium(MediumCommand::Choose {
                            answer: KaeriIsFor::Both,
                            delete_downloads: false,
                        }));
                    }
                });
                column.append(&on);
                (column.upcast(), t::action_done(), true)
            }
            StoreStep::Empty(report) => (
                note(
                    Mark::Info,
                    &t::store_empty(&report.name),
                    Some(&t::store_empty_line(&owner(report))),
                )
                .upcast(),
                t::action_done(),
                true,
            ),
            StoreStep::Unreachable(host) => {
                let owner = self
                    .store
                    .borrow()
                    .as_ref()
                    .and_then(|s| s.owners.first().cloned())
                    .unwrap_or_else(|| host.clone());
                (
                    note(
                        Mark::Failed,
                        &t::store_err_unreachable(host),
                        Some(&t::store_err_unreachable_line(&owner)),
                    )
                    .upcast(),
                    t::action_try_again(),
                    self.store.borrow().is_some() || self.invite_link().is_some(),
                )
            }
        };
        self.primary.set_label(&label);
        self.primary.set_sensitive(on && !self.installing.get());
        self.result.show(&shown);
    }

    /// For reading, then for watching: each plugin with Install, or Installed once it is.
    fn groups(&self, column: &gtk::Box, report: &StoreReport) {
        let plugins: Vec<StorePluginRow> = self
            .ctx
            .hub
            .known
            .borrow()
            .stores
            .iter()
            .find(|s| s.id == report.store)
            .map(|s| s.plugins.clone())
            .unwrap_or_default();
        for (title, ids) in [
            (t::store_group_reading(), &report.for_reading),
            (t::store_group_watching(), &report.for_watching),
        ] {
            if ids.is_empty() {
                continue;
            }
            let group = adw::PreferencesGroup::builder().title(title).build();
            for id in ids {
                let Some(plugin) = plugins.iter().find(|p| p.id == *id) else { continue };
                group.add(&self.plugin_row(&report.store, plugin));
            }
            column.append(&group);
        }
        if report.other_medium > 0 && !report.other_is_reading {
            column.append(&widgets::haze(
                &t::store_other_medium_more(i64::from(report.other_medium)),
                "caption",
            ));
        }
    }

    /// The plugins for the medium this device doesn't show, listed without Install.
    fn other_group(&self, column: &gtk::Box, report: &StoreReport) {
        let plugins: Vec<StorePluginRow> = self
            .ctx
            .hub
            .known
            .borrow()
            .stores
            .iter()
            .find(|s| s.id == report.store)
            .map(|s| s.plugins.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|p| p.books == report.other_is_reading)
            .collect();
        if plugins.is_empty() {
            return;
        }
        let title = if report.other_is_reading {
            t::store_group_reading()
        } else {
            t::store_group_watching()
        };
        let group = adw::PreferencesGroup::builder().title(title).build();
        for plugin in &plugins {
            let hosts = t::linux_store_plugin_hosts(plugin.hosts.len() as i64);
            let row = adw::ActionRow::builder()
                .title(glib::markup_escape_text(&plugin.name))
                .subtitle(glib::markup_escape_text(&t::store_plugin_row(
                    &plugin.version,
                    &hosts,
                )))
                .build();
            row.add_prefix(&tile("application-x-addon-symbolic"));
            group.add(&row);
        }
        column.append(&group);
    }

    fn plugin_row(&self, store: &str, plugin: &StorePluginRow) -> adw::ActionRow {
        let hosts = t::linux_store_plugin_hosts(plugin.hosts.len() as i64);
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&plugin.name))
            .subtitle(glib::markup_escape_text(&t::store_plugin_row(&plugin.version, &hosts)))
            .build();
        row.add_prefix(&tile("application-x-addon-symbolic"));
        if plugin.status == StorePluginStatus::Installed {
            let done = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            done.set_valign(gtk::Align::Center);
            done.append(&widgets::haze(&t::store_installed_row(), ""));
            let check = gtk::Image::from_icon_name("object-select-symbolic");
            check.add_css_class("came-in-found");
            done.append(&check);
            row.add_suffix(&done);
        } else {
            let install = gtk::Button::with_label(&t::store_install());
            install.set_valign(gtk::Align::Center);
            let (hub, store, id) =
                (Rc::downgrade(&self.ctx.hub), store.to_owned(), plugin.id.clone());
            install.connect_clicked(move |button| {
                button.set_sensitive(false);
                if let Some(hub) = hub.upgrade() {
                    hub.send(Command::InstallPlugin {
                        store: store.clone(),
                        plugin: id.clone(),
                    });
                }
            });
            row.add_suffix(&install);
        }
        row
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kavita_link_is_read_as_the_engine_reads_it() {
        let link = kavita("https://books.example/api/opds/abcd1234ef56").unwrap();
        assert_eq!(link.host, "books.example");
        assert_eq!(link.last4, "ef56");
        assert_eq!(link.root, "https://books.example");
        let proxied = kavita("books.example:5000/kavita/API/OPDS/key_key-key9").unwrap();
        assert_eq!(proxied.root, "https://books.example:5000/kavita");
        assert_eq!(proxied.last4, "key9");
        assert_eq!(kavita("https://books.example/api/opds/short"), None);
        assert_eq!(kavita("https://me@books.example/api/opds/abcd1234ef56"), None);
        assert_eq!(kavita("https://books.example/opds"), None);
        assert_eq!(kavita("books example/api/opds/abcd1234ef56"), None);
    }

    #[test]
    fn an_address_parses_once_it_could_name_a_server() {
        assert!(!looks_like_address(""));
        assert!(!looks_like_address("bo"));
        assert!(!looks_like_address("books"));
        assert!(looks_like_address("books.home.example"));
        assert!(looks_like_address("localhost"));
        assert!(looks_like_address("10.0.0.2:25600"));
        assert!(looks_like_address("https://books.example"));
        assert!(!looks_like_address("books .example"));
    }

    #[test]
    fn the_host_is_the_address_without_scheme_port_or_path() {
        assert_eq!(host_of("https://books.home.example:25600/opds"), "books.home.example");
        assert_eq!(host_of("books.home.example"), "books.home.example");
        assert_eq!(host_of("http://me@10.0.0.2:8080"), "10.0.0.2");
    }
}
