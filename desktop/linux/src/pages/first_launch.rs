//! First run: five short pages in a large dialog (a bottom sheet on a narrow window), then the
//! app. Welcome; Already use Kaeri on another device?; Watch, read, or both?; Bring in your
//! lists; and where the shows or books come from. Each page says "N of 5" over its title, keeps
//! its column narrow and its one amber action at the foot. Closing the dialog skips the rest, and
//! each step is in Settings afterwards. It never mentions torrents.
//!
//! The join page shows a code for another device, and once paired lists what that device brings
//! (its answer to Watch, read, or both, its sign-ins, plugin stores and book servers, each a
//! switch). Continue takes what was left on and skips the pages that answered.

use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    rc::Rc,
    time::{Duration, Instant},
};

use adw::prelude::*;
use gtk::glib;
use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    event::Event,
    group::{
        FirstRunPage, JoinedSetup, SyncCommand, SyncCopy, SyncFolderKind, SyncStoreOffer,
        SyncStoreProblem,
    },
    medium::KaeriIsFor,
};

use crate::{
    nagare,
    reading::{came_in, medium},
    states::SILENT,
    widgets,
    window::Ctx,
};

/// A code is good for this long.
const CODE_SECONDS: i64 = 120;
/// With nothing after this long, the hint that both need the same Wi-Fi.
const HINT_AFTER: Duration = Duration::from_secs(30);
/// The paired device's setup gets this long to arrive before the page goes on without it.
const SETUP_WAIT: Duration = Duration::from_secs(15);
/// Stores asked to join get this long to answer.
const JOIN_WAIT: Duration = Duration::from_secs(15);

/// The pages in order, by tag.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Welcome,
    Join,
    Medium,
    Lists,
    Sources,
}

/// Where the join page stands.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Stage {
    #[default]
    Ask,
    Code,
    Paired,
}

/// The join page's state, read against the clock as it ticks.
#[derive(Default)]
struct Join {
    stage: Stage,
    code_at: Option<Instant>,
    paired_at: Option<Instant>,
    device: Option<String>,
    /// The group's size when the code showed: one more is the pairing.
    started_with: usize,
    trackers: Vec<String>,
    setup: Option<JoinedSetup>,
    medium: Option<KaeriIsFor>,
    sign_ins: bool,
    stores_off: HashSet<String>,
    servers_off: HashSet<String>,
    asked: HashSet<String>,
    asked_at: Option<Instant>,
    answered: HashSet<String>,
    refused: Vec<(String, SyncStoreProblem)>,
    presses: u32,
    /// Continue went on to the next page.
    gone: bool,
}

impl Join {
    fn since(at: Option<Instant>) -> Duration {
        at.map_or(Duration::ZERO, |at| at.elapsed())
    }

    fn gave_up(&self) -> bool {
        self.stage == Stage::Paired
            && self.setup.is_none()
            && Self::since(self.paired_at) >= SETUP_WAIT
    }

    fn looking(&self) -> bool {
        self.stage == Stage::Paired
            && self.setup.is_none()
            && !self.gave_up()
            && Self::since(self.paired_at) >= SILENT
    }

    fn landed(&self) -> bool {
        self.stage == Stage::Paired && (self.setup.is_some() || self.gave_up())
    }

    fn pending(&self) -> HashSet<String> {
        if Self::since(self.asked_at) >= JOIN_WAIT {
            return HashSet::new();
        }
        self.asked.difference(&self.answered).cloned().collect()
    }

    fn working(&self) -> bool {
        !self.pending().is_empty() && Self::since(self.asked_at) >= SILENT
    }

    fn leaving(&self) -> bool {
        self.presses > 0
            && self.pending().is_empty()
            && !self.refused.iter().any(|(name, _)| self.asked.contains(name))
    }

    fn refusal(&self, name: &str) -> Option<SyncStoreProblem> {
        self.refused.iter().rev().find(|(n, _)| n == name).map(|(_, p)| *p)
    }

    fn device_name(&self) -> String {
        self.setup
            .as_ref()
            .map(|s| s.device_name.clone())
            .filter(|n| !n.is_empty())
            .or_else(|| self.device.clone())
            .unwrap_or_else(t::pair_other_device)
    }

    /// The first-run pages after this one still to ask.
    fn pages_left(&self) -> Vec<FirstRunPage> {
        let sources = self.setup.as_ref().is_some_and(|s| {
            s.stores.iter().any(|o| !self.stores_off.contains(&o.name))
                || s.servers.iter().any(|o| !self.servers_off.contains(&o.id))
        });
        let done: Vec<FirstRunPage> = self
            .setup
            .as_ref()
            .map(|s| s.answered.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|page| match page {
                FirstRunPage::Medium => true,
                FirstRunPage::Lists => self.sign_ins,
                FirstRunPage::Sources => sources,
            })
            .collect();
        [FirstRunPage::Medium, FirstRunPage::Lists, FirstRunPage::Sources]
            .into_iter()
            .filter(|page| !done.contains(page))
            .collect()
    }
}

/// The code page's widgets.
struct CodeParts {
    qr_slot: gtk::Box,
    qr_code: RefCell<String>,
    digits: Vec<gtk::Label>,
    digits_box: gtk::Box,
    good_for: gtk::Label,
    status: gtk::Box,
    status_icon: gtk::Stack,
    status_text: gtk::Label,
    hint: gtk::Box,
    again: gtk::Button,
    /// The waiting line came in.
    status_shown: Cell<bool>,
}

/// The paired page's widgets.
struct PairedParts {
    title: gtk::Label,
    heading: gtk::Label,
    list: gtk::ListBox,
    notes: gtk::Box,
    go: gtk::Button,
    go_label: gtk::Stack,
    /// What the list was drawn from, so a tick redraws only when it changed.
    drawn: RefCell<String>,
    /// The rows drawn before, by title: only a new one comes in with a fade.
    seen: RefCell<HashSet<String>>,
}

struct Run {
    ctx: Rc<Ctx>,
    dialog: adw::Dialog,
    pages: adw::NavigationView,
    steps: Vec<Step>,
    answer: Cell<Option<KaeriIsFor>>,
    join: RefCell<Join>,
    code: RefCell<Option<Rc<CodeParts>>>,
    paired: RefCell<Option<Rc<PairedParts>>>,
    added: RefCell<Vec<came_in::Added>>,
    added_list: RefCell<Option<gtk::ListBox>>,
    tick: RefCell<Option<glib::SourceId>>,
}

impl Run {
    fn step(&self, step: Step) -> String {
        let at = self.steps.iter().position(|s| *s == step).unwrap_or(0);
        t::onboarding_step(at as i64 + 1, self.steps.len() as i64)
    }

    fn next_after(&self, step: Step) -> Option<Step> {
        let at = self.steps.iter().position(|s| *s == step)?;
        self.steps.get(at + 1).copied()
    }

    fn answer(&self) -> KaeriIsFor {
        self.answer
            .get()
            .or_else(|| self.ctx.hub.known.borrow().medium.as_ref().map(|m| m.answer))
            .unwrap_or(KaeriIsFor::Watch)
    }
}

/// Open first run over the window.
pub fn open(ctx: &Rc<Ctx>) {
    medium::first_run_opens();
    let reading =
        ctx.hub.known.borrow().medium.as_ref().is_some_and(|m| m.reading_available && !m.fixed);
    let steps = if reading {
        vec![Step::Welcome, Step::Join, Step::Medium, Step::Lists, Step::Sources]
    } else {
        vec![Step::Welcome, Step::Join, Step::Lists, Step::Sources]
    };
    let pages = adw::NavigationView::new();
    let dialog = adw::Dialog::builder()
        .title(t::onboarding_welcome_to())
        .content_width(900)
        .content_height(640)
        .child(&pages)
        .build();
    dialog.add_css_class("first-run");
    let run = Rc::new(Run {
        ctx: Rc::clone(ctx),
        dialog: dialog.clone(),
        pages: pages.clone(),
        steps,
        answer: Cell::new(None),
        join: RefCell::default(),
        code: RefCell::default(),
        paired: RefCell::default(),
        added: RefCell::default(),
        added_list: RefCell::default(),
        tick: RefCell::default(),
    });
    pages.add(&welcome(&run));
    pages.connect_popped({
        let run = Rc::downgrade(&run);
        move |_, page| {
            // Back from the code: it stops, and nothing is held.
            if let Some(run) = run.upgrade()
                && page.tag().as_deref() == Some("join-code")
                && run.join.borrow().stage == Stage::Code
            {
                run.ctx.hub.send(Command::PairStop);
                *run.join.borrow_mut() = Join::default();
                run.code.replace(None);
            }
        }
    });
    let heard = ctx.hub.listen({
        let run = Rc::downgrade(&run);
        move |event| {
            if let Some(run) = run.upgrade() {
                heard(&run, event);
            }
        }
    });
    came_in::on_added(Some(Rc::new({
        let run = Rc::downgrade(&run);
        move |added| {
            if let Some(run) = run.upgrade() {
                run.added.borrow_mut().push(added);
                draw_added(&run);
            }
        }
    })));
    dialog.connect_closed({
        let run = Rc::clone(&run);
        move |_| {
            run.ctx.hub.forget(heard);
            if let Some(id) = run.tick.borrow_mut().take() {
                id.remove();
            }
            if run.join.borrow().stage == Stage::Code {
                run.ctx.hub.send(Command::PairStop);
            }
            came_in::show_in(None);
            came_in::on_added(None);
            medium::first_run_closed(&run.ctx);
            // Joined another device: the rest syncs from there, said once over Home.
            let join = run.join.borrow();
            if join.stage == Stage::Paired && join.presses > 0 {
                run.ctx.toast(&t::first_join_syncing(&join.device_name()));
            }
        }
    });
    dialog.present(Some(&ctx.window));
}

/// First run on its last page, as a shot shows it.
pub fn open_sources(ctx: &Rc<Ctx>) {
    open(ctx);
    if let Some(run) = shown_run(ctx) {
        push(&run, Step::Sources);
    }
}

/// The run behind the dialog showing, for the shots.
fn shown_run(ctx: &Rc<Ctx>) -> Option<Rc<Run>> {
    let dialog = ctx.window.visible_dialog()?;
    RUNS.with_borrow(|runs| {
        runs.iter().filter_map(std::rc::Weak::upgrade).find(|run| run.dialog == dialog)
    })
}

thread_local! {
    static RUNS: RefCell<Vec<std::rc::Weak<Run>>> = const { RefCell::new(Vec::new()) };
}

// ---------------------------------------------------------------------------------------------
// The frame every page takes

/// A page: its step over a big title and a line, the body in a narrow column, and the foot (the
/// one amber action, with a quieter one under it) fixed at the bottom.
fn frame(
    tag: &str,
    step: &str,
    title: &str,
    line: Option<&str>,
    body: &impl IsA<gtk::Widget>,
    foot: &gtk::Box,
) -> (adw::NavigationPage, gtk::Label) {
    let header = adw::HeaderBar::new();
    header.set_show_title(false);
    header.add_css_class("flat");
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.set_margin_top(20);
    column.set_margin_bottom(24);
    column.set_margin_start(18);
    column.set_margin_end(18);
    let step = gtk::Label::new(Some(step));
    step.add_css_class("caption");
    step.add_css_class("haze");
    column.append(&step);
    let title = gtk::Label::new(Some(title));
    title.add_css_class("first-title");
    title.set_wrap(true);
    title.set_justify(gtk::Justification::Center);
    title.set_max_width_chars(28);
    title.set_margin_top(4);
    title.set_accessible_role(gtk::AccessibleRole::Heading);
    column.append(&title);
    if let Some(line) = line {
        let line = gtk::Label::new(Some(line));
        line.add_css_class("haze");
        line.set_wrap(true);
        line.set_justify(gtk::Justification::Center);
        line.set_max_width_chars(60);
        line.set_margin_top(8);
        column.append(&line);
    }
    body.as_ref().set_margin_top(24);
    column.append(body);
    let clamp = adw::Clamp::builder()
        .maximum_size(520)
        .tightening_threshold(520)
        .child(&column)
        .build();
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&clamp)
        .build();
    foot.set_halign(gtk::Align::Center);
    foot.set_margin_top(12);
    foot.set_margin_bottom(36);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&scroll);
    content.append(foot);
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&content));
    let page =
        adw::NavigationPage::builder().tag(tag).title(title.label()).child(&view).build();
    (page, title)
}

fn foot() -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Vertical, 12)
}

fn amber(label: &str) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.add_css_class("pill");
    button.add_css_class("suggested-action");
    button.set_halign(gtk::Align::Center);
    button
}

fn quiet(label: &str) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.add_css_class("flat");
    button.set_halign(gtk::Align::Center);
    button
}

/// A row with a tile, a title and a line, and a chevron: it opens something.
fn nav_row(icon: &str, title: &str, line: &str) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(glib::markup_escape_text(title))
        .subtitle(glib::markup_escape_text(line))
        .activatable(true)
        .build();
    row.add_prefix(&came_in::tile(icon));
    row.add_suffix(&widgets::chevron());
    row
}

/// A row an add of an add form shows: the form for the page `name`, inside first run.
pub fn form_page(
    name: &str,
    step: &str,
    title: &str,
    column: &gtk::Box,
    primary: &gtk::Button,
) -> adw::NavigationPage {
    column.set_spacing(14);
    let foot = foot();
    foot.append(primary);
    frame(name, step, title, None, column, &foot).0
}

fn push(run: &Rc<Run>, step: Step) {
    let page = match step {
        Step::Welcome => welcome(run),
        Step::Join => join(run),
        Step::Medium => medium_page(run),
        Step::Lists => lists(run),
        Step::Sources => sources(run),
    };
    run.pages.push(&page);
}

fn go_on(run: &Rc<Run>, from: Step) {
    match run.next_after(from) {
        Some(step) => push(run, step),
        None => {
            run.dialog.close();
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 1 Welcome

fn welcome(run: &Rc<Run>) -> adw::NavigationPage {
    RUNS.with_borrow_mut(|runs| {
        runs.retain(|r| r.strong_count() > 0);
        runs.push(Rc::downgrade(run));
    });
    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let picture = nagare::picture(5);
    picture.set_halign(gtk::Align::Center);
    body.append(&picture);
    let note = gtk::Label::new(Some(&t::onboarding_welcome_note()));
    note.add_css_class("haze");
    note.add_css_class("caption");
    note.set_wrap(true);
    note.set_justify(gtk::Justification::Center);
    note.set_margin_top(12);
    body.append(&note);
    let foot = foot();
    let next = amber(&t::onboarding_continue());
    next.connect_clicked({
        let run = Rc::downgrade(run);
        move |_| {
            if let Some(run) = run.upgrade() {
                go_on(&run, Step::Welcome);
            }
        }
    });
    foot.append(&next);
    let (page, _) = frame(
        "welcome",
        &run.step(Step::Welcome),
        &t::onboarding_welcome_to(),
        Some(&t::onboarding_welcome_line()),
        &body,
        &foot,
    );
    page
}

// ---------------------------------------------------------------------------------------------
// 2 Already use Kaeri on another device?

fn join(run: &Rc<Run>) -> adw::NavigationPage {
    let list = widgets::boxed_list();
    let pair = nav_row("phone-symbolic", &t::first_setup_other(), &t::first_other_pair_line());
    pair.connect_activated({
        let run = Rc::downgrade(run);
        move |_| {
            if let Some(run) = run.upgrade() {
                show_code(&run);
            }
        }
    });
    list.append(&pair);
    let restore = nav_row(
        "weather-overcast-symbolic",
        &t::first_other_restore(),
        &t::first_other_restore_line(),
    );
    restore.connect_activated({
        let run = Rc::downgrade(run);
        move |_| {
            if let Some(run) = run.upgrade() {
                restore_instead(&run);
            }
        }
    });
    list.append(&restore);
    let foot = foot();
    let no = amber(&t::first_other_no());
    no.connect_clicked({
        let run = Rc::downgrade(run);
        move |_| {
            if let Some(run) = run.upgrade() {
                go_on(&run, Step::Join);
            }
        }
    });
    foot.append(&no);
    let (page, _) = frame(
        "join",
        &run.step(Step::Join),
        &t::first_other_title(),
        Some(&t::first_other_line()),
        &list,
        &foot,
    );
    page
}

fn restore_instead(run: &Rc<Run>) {
    run.dialog.close();
    crate::sync::dialogs::picker(&run.ctx);
}

/// The code this device shows, for the other device to enter or scan. Sign-ins, stores and the
/// setup are asked of whichever device pairs.
fn show_code(run: &Rc<Run>) {
    ask_code(run);
    run.pages.push(&code_page(run));
    start_tick(run);
}

/// Ask for a code, and ask whichever device pairs for its sign-ins, stores and setup.
fn ask_code(run: &Rc<Run>) {
    let ctx = &run.ctx;
    {
        let mut join = run.join.borrow_mut();
        *join = Join {
            stage: Stage::Code,
            code_at: Some(Instant::now()),
            sign_ins: true,
            started_with: ctx
                .hub
                .known
                .borrow()
                .sync_group
                .as_ref()
                .map_or(0, |s| s.devices.len()),
            ..Join::default()
        };
    }
    if let Some(parts) = run.code.borrow().as_ref() {
        parts.status_shown.set(false);
        parts.status.set_opacity(0.0);
    }
    ctx.hub.known.borrow_mut().invite = None;
    let mut copy = vec![SyncCopy::Trackers];
    copy.extend(
        [SyncFolderKind::GoogleDrive, SyncFolderKind::Dropbox, SyncFolderKind::OneDrive]
            .into_iter()
            .map(|service| SyncCopy::Cloud { service }),
    );
    ctx.hub.send(Command::SyncGroup(SyncCommand::CopySignIns {
        device: None,
        copy,
        address: None,
    }));
    ctx.hub.send(Command::SyncGroup(SyncCommand::AskSetupFrom { device: None, address: None }));
    ctx.hub.send(Command::SyncGroup(SyncCommand::JoinStoresFrom {
        device: None,
        stores: Vec::new(),
        address: None,
    }));
    ctx.hub
        .send(Command::PairInvite { device_name: crate::sync::dialogs::typed_device_name() });
}

fn code_page(run: &Rc<Run>) -> adw::NavigationPage {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 14);
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 28);
    let qr_slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    qr_slot.add_css_class("first-qr");
    qr_slot.set_valign(gtk::Align::Start);
    qr_slot.append(&crate::sync::dialogs::qr_area("", 100));
    top.append(&qr_slot);
    let right = gtk::Box::new(gtk::Orientation::Vertical, 12);
    right.set_valign(gtk::Align::Center);
    let digits_box = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    digits_box.add_css_class("first-digits");
    let mut digits = Vec::new();
    for at in 0..6 {
        if at == 3 {
            let gap = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            gap.set_size_request(12, -1);
            digits_box.append(&gap);
        }
        let digit = gtk::Label::new(Some("\u{2013}"));
        digit.add_css_class("first-digit");
        digit.add_css_class("numeric");
        digits_box.append(&digit);
        digits.push(digit);
    }
    right.append(&digits_box);
    let good_for = widgets::haze("", "caption");
    good_for.add_css_class("numeric");
    good_for.set_xalign(0.0);
    right.append(&good_for);
    top.append(&right);
    body.append(&top);
    let status = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    status.add_css_class("first-status");
    let status_icon = gtk::Stack::new();
    status_icon.add_named(&adw::Spinner::new(), Some("waiting"));
    let warn = gtk::Image::from_icon_name("dialog-warning-symbolic");
    warn.add_css_class("came-in-failed");
    status_icon.add_named(&warn, Some("expired"));
    status_icon.set_size_request(18, 18);
    status.append(&status_icon);
    let status_text = gtk::Label::new(None);
    status_text.add_css_class("heading");
    status_text.set_xalign(0.0);
    status_text.set_wrap(true);
    status.append(&status_text);
    status.set_opacity(0.0);
    body.append(&status);
    let hint = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    hint.add_css_class("first-status");
    let info = gtk::Image::from_icon_name("help-about-symbolic");
    info.set_size_request(18, 18);
    hint.append(&info);
    let hint_text = gtk::Label::new(Some(&t::pair_nothing_yet()));
    hint_text.add_css_class("heading");
    hint_text.set_wrap(true);
    hint_text.set_xalign(0.0);
    hint.append(&hint_text);
    hint.set_visible(false);
    body.append(&hint);
    let foot = foot();
    let again = amber(&t::pair_show_code());
    again.set_visible(false);
    again.connect_clicked({
        let run = Rc::downgrade(run);
        move |_| {
            if let Some(run) = run.upgrade() {
                // A fresh code, in place.
                ask_code(&run);
                refresh(&run);
            }
        }
    });
    foot.append(&again);
    let restore = quiet(&t::first_restore());
    restore.connect_clicked({
        let run = Rc::downgrade(run);
        move |_| {
            if let Some(run) = run.upgrade() {
                restore_instead(&run);
            }
        }
    });
    foot.append(&restore);
    let (page, _) = frame(
        "join-code",
        &run.step(Step::Join),
        &t::pair_show_title(),
        Some(&t::first_setup_line()),
        &body,
        &foot,
    );
    run.code.replace(Some(Rc::new(CodeParts {
        qr_slot,
        qr_code: RefCell::default(),
        digits,
        digits_box,
        good_for,
        status,
        status_icon,
        status_text,
        hint,
        again,
        status_shown: Cell::new(false),
    })));
    page
}

/// Redraw what the clock moves: the countdown, the waiting line, the hint, and on the paired
/// page what arrived and Continue.
fn start_tick(run: &Rc<Run>) {
    if run.tick.borrow().is_some() {
        return;
    }
    let id = glib::timeout_add_local(Duration::from_millis(250), {
        let run = Rc::downgrade(run);
        move || {
            let Some(run) = run.upgrade() else { return glib::ControlFlow::Break };
            refresh(&run);
            glib::ControlFlow::Continue
        }
    });
    run.tick.replace(Some(id));
    refresh(run);
}

fn refresh(run: &Rc<Run>) {
    let stage = run.join.borrow().stage;
    match stage {
        Stage::Code => refresh_code(run),
        Stage::Paired => refresh_paired(run),
        Stage::Ask => {}
    }
}

fn refresh_code(run: &Rc<Run>) {
    let Some(parts) = run.code.borrow().clone() else { return };
    let invite = run.ctx.hub.known.borrow().invite.clone();
    let since = Join::since(run.join.borrow().code_at);
    let Some((code, port, until)) = invite else {
        return;
    };
    let left = until - glib::real_time() / 1_000_000;
    let expired = left <= 0;
    for (digit, c) in parts.digits.iter().zip(code.chars()) {
        digit.set_label(&c.to_string());
    }
    parts
        .digits_box
        .update_property(&[gtk::accessible::Property::Label(&crate::sync::spaced_code(&code))]);
    if *parts.qr_code.borrow() != code {
        let host = crate::sync::local_address().unwrap_or_else(|| "127.0.0.1".to_owned());
        let link = format!("kaeri://pair/{code}@{host}:{port}");
        crate::reading::clear(&parts.qr_slot);
        parts.qr_slot.append(&crate::sync::dialogs::qr_area(&link, 100));
        parts.qr_code.replace(code.clone());
    }
    parts.good_for.set_label(&t::pair_good_for(&crate::sync::minutes_seconds(
        left.clamp(0, CODE_SECONDS),
    )));
    for dimmed in [
        parts.qr_slot.upcast_ref::<gtk::Widget>(),
        parts.digits_box.upcast_ref(),
        parts.good_for.upcast_ref(),
    ] {
        if expired {
            dimmed.add_css_class("first-expired");
        } else {
            dimmed.remove_css_class("first-expired");
        }
    }
    if expired {
        parts.status_icon.set_visible_child_name("expired");
        parts.status_text.set_label(&t::pair_err_expired());
        parts.status.set_opacity(1.0);
        parts.status_shown.set(true);
        parts.hint.set_visible(false);
        parts.again.set_visible(true);
    } else {
        parts.status_icon.set_visible_child_name("waiting");
        parts.status_text.set_label(&t::pair_waiting());
        if since >= SILENT && !parts.status_shown.replace(true) {
            if crate::motion::reduced() {
                parts.status.set_opacity(1.0);
            } else {
                let _ = crate::motion::crossfade_in(&parts.status);
            }
        }
        parts.hint.set_visible(since >= HINT_AFTER);
        parts.again.set_visible(false);
    }
}

/// The other device paired: the page crossfades to what it brings.
fn paired(run: &Rc<Run>, device: Option<String>) {
    {
        let mut join = run.join.borrow_mut();
        if join.stage != Stage::Code {
            if device.is_some() && join.device.is_none() {
                join.device = device;
            }
            return;
        }
        join.stage = Stage::Paired;
        join.paired_at = Some(Instant::now());
        if device.is_some() {
            join.device = device;
        }
    }
    let page = paired_page(run);
    // The code is spent: the paired page takes its place, with no way back to it.
    run.pages.replace(&[welcome_like(run), join_like(run), page]);
    start_tick(run);
}

/// The pages under the paired one, so Back is never offered to a spent code.
fn welcome_like(run: &Rc<Run>) -> adw::NavigationPage {
    run.pages.find_page("welcome").unwrap_or_else(|| welcome(run))
}

fn join_like(run: &Rc<Run>) -> adw::NavigationPage {
    run.pages.find_page("join").unwrap_or_else(|| join(run))
}

fn paired_page(run: &Rc<Run>) -> adw::NavigationPage {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let heading = gtk::Label::new(None);
    heading.add_css_class("heading");
    heading.set_xalign(0.0);
    body.append(&heading);
    let list = widgets::boxed_list();
    body.append(&list);
    let notes = gtk::Box::new(gtk::Orientation::Vertical, 6);
    notes.set_margin_top(6);
    body.append(&notes);
    let foot = foot();
    let go = amber("");
    let go_label = gtk::Stack::new();
    go_label.set_transition_type(gtk::StackTransitionType::Crossfade);
    go_label.set_transition_duration(if crate::motion::reduced() { 0 } else { 200 });
    go_label.add_named(&gtk::Label::new(Some(&t::onboarding_continue())), Some("words"));
    go_label.add_named(&adw::Spinner::new(), Some("working"));
    go.set_child(Some(&go_label));
    go.update_property(&[gtk::accessible::Property::Label(&t::onboarding_continue())]);
    go.connect_clicked({
        let run = Rc::downgrade(run);
        move |_| {
            if let Some(run) = run.upgrade() {
                carry_on(&run);
            }
        }
    });
    foot.append(&go);
    let device = run.join.borrow().device_name();
    let (page, title) = frame(
        "join-paired",
        &run.step(Step::Join),
        &t::pair_ok(&device.replace(' ', "\u{a0}")),
        Some(&t::first_ok_line()),
        &body,
        &foot,
    );
    page.set_can_pop(false);
    run.paired.replace(Some(Rc::new(PairedParts {
        title,
        heading,
        list,
        notes,
        go,
        go_label,
        drawn: RefCell::default(),
        seen: RefCell::default(),
    })));
    page
}

fn medium_words(answer: KaeriIsFor) -> String {
    match answer {
        KaeriIsFor::Watch => t::medium_answer_watch(),
        KaeriIsFor::Read => t::medium_answer_read(),
        KaeriIsFor::Both => t::medium_answer_both(),
    }
}

fn refresh_paired(run: &Rc<Run>) {
    let Some(parts) = run.paired.borrow().clone() else { return };
    let join = run.join.borrow();
    let device = join.device_name();
    parts.title.set_label(&t::pair_ok(&device.replace(' ', "\u{a0}")));
    parts.heading.set_label(&t::first_join_group(&device));
    let landed = join.landed();
    let working = join.working();
    parts.go.set_sensitive(landed && !working);
    parts.go_label.set_visible_child_name(if working { "working" } else { "words" });
    let leaving = join.leaving();
    // What the rows are drawn from; redrawn only when it moved.
    let key = format!(
        "{device}|{}|{}|{:?}|{:?}",
        join.looking(),
        join.setup.is_some(),
        join.trackers,
        join.refused.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
    );
    drop(join);
    if *parts.drawn.borrow() != key {
        parts.drawn.replace(key);
        draw_paired(run, &parts);
    }
    if leaving {
        leave_paired(run);
    }
}

fn draw_paired(run: &Rc<Run>, parts: &PairedParts) {
    let join = run.join.borrow();
    let device = join.device_name();
    parts.list.remove_all();
    crate::reading::clear(&parts.notes);
    let setup = join.setup.clone();
    // Kaeri is for, as on the other device, with a pop-up to answer otherwise.
    if let Some(answer) = setup.as_ref().and_then(|s| s.medium) {
        let chosen = join.medium.unwrap_or(answer);
        let line = match answer {
            KaeriIsFor::Watch => t::linux_join_medium_line_watch(&device),
            KaeriIsFor::Read => t::first_join_medium_line(&device),
            KaeriIsFor::Both => t::linux_join_medium_line_both(&device),
        };
        let words: Vec<String> = medium::ALL.into_iter().map(medium_words).collect();
        let model = gtk::StringList::new(&words.iter().map(String::as_str).collect::<Vec<_>>());
        let row = adw::ComboRow::builder()
            .title(glib::markup_escape_text(&t::first_join_medium()))
            .subtitle(glib::markup_escape_text(&line))
            .model(&model)
            .selected(medium::ALL.iter().position(|a| *a == chosen).unwrap_or(0) as u32)
            .build();
        row.connect_selected_notify({
            let run = Rc::downgrade(run);
            move |row| {
                if let Some(run) = run.upgrade() {
                    run.join.borrow_mut().medium =
                        medium::ALL.get(row.selected() as usize).copied();
                }
            }
        });
        parts.list.append(&row);
    }
    // Copy sign-ins: on, as first run asked for them from the start.
    let trackers =
        setup.as_ref().map_or_else(|| join.trackers.clone(), |s| s.signed_in.clone());
    if !trackers.is_empty() {
        let names: Vec<String> =
            trackers.iter().map(|id| crate::say::tracker_name(id).to_owned()).collect();
        let row = adw::SwitchRow::builder()
            .title(glib::markup_escape_text(&t::pair_copy_signins(&device)))
            .subtitle(glib::markup_escape_text(&t::first_join_signins_line(&names.join(", "))))
            .active(join.sign_ins)
            .build();
        row.connect_active_notify({
            let run = Rc::downgrade(run);
            move |row| {
                if let Some(run) = run.upgrade() {
                    run.join.borrow_mut().sign_ins = row.is_active();
                }
            }
        });
        parts.list.append(&row);
    }
    for offer in setup.as_ref().map(|s| s.stores.clone()).unwrap_or_default() {
        parts.list.append(&store_row(run, &join, &offer, &device));
    }
    for server in setup.as_ref().map(|s| s.servers.clone()).unwrap_or_default() {
        if server.added {
            continue;
        }
        let row = adw::SwitchRow::builder()
            .title(glib::markup_escape_text(&t::first_join_server(&server.host)))
            .subtitle(glib::markup_escape_text(&t::first_join_server_line(
                &came_in::kind_words(server.kind),
                &device,
                &t::secure_store_linux(),
            )))
            .active(!join.servers_off.contains(&server.id))
            .build();
        row.connect_active_notify({
            let (run, id) = (Rc::downgrade(run), server.id.clone());
            move |row| {
                if let Some(run) = run.upgrade() {
                    let mut join = run.join.borrow_mut();
                    if row.is_active() {
                        join.servers_off.remove(&id);
                    } else {
                        join.servers_off.insert(id.clone());
                    }
                }
            }
        });
        parts.list.append(&row);
    }
    if join.looking() {
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&t::first_join_looking(&device)))
            .build();
        row.add_suffix(&adw::Spinner::new());
        parts.list.append(&row);
    }
    parts.list.set_visible(parts.list.first_child().is_some());
    parts.heading.set_visible(parts.list.first_child().is_some());
    if let Some(setup) = &setup {
        if setup.folders > 0 {
            parts.notes.append(&note(&t::first_join_folders(&device)));
        }
        if !setup.answered.is_empty() && join.pages_left().is_empty() {
            parts.notes.append(&note(&t::first_join_steps_done()));
        }
    }
    // A row that just arrived comes in; the ones already there stay as they are.
    let mut seen = parts.seen.borrow_mut();
    let mut child = parts.list.first_child();
    while let Some(row) = child {
        child = row.next_sibling();
        let Some(title) =
            row.downcast_ref::<adw::PreferencesRow>().map(|r| r.title().to_string())
        else {
            continue;
        };
        if seen.insert(title) && !crate::motion::reduced() {
            let _ = crate::motion::crossfade_in(&row);
        }
    }
}

fn store_row(run: &Rc<Run>, join: &Join, offer: &SyncStoreOffer, device: &str) -> gtk::Widget {
    let title = glib::markup_escape_text(&t::first_join_store(&offer.name));
    if let Some(problem) = join.refusal(&offer.name) {
        let line = match problem {
            SyncStoreProblem::Refused => t::first_join_store_refused(&offer.name, device),
            SyncStoreProblem::Offline => t::sync_store_err_offline(&offer.name, device),
            SyncStoreProblem::NotAllowed => t::sync_store_err_not_allowed(&offer.name, device),
        };
        let row = adw::ActionRow::builder()
            .title(title)
            .subtitle(glib::markup_escape_text(&line))
            .subtitle_lines(0)
            .build();
        let warn = gtk::Image::from_icon_name("dialog-warning-symbolic");
        warn.add_css_class("came-in-failed");
        row.add_suffix(&warn);
        return row.upcast();
    }
    let row = adw::SwitchRow::builder()
        .title(title)
        .subtitle(glib::markup_escape_text(&t::first_join_store_line(
            device,
            &crate::sync::kind(),
        )))
        .active(!join.stores_off.contains(&offer.name))
        .build();
    row.connect_active_notify({
        let (run, name) = (Rc::downgrade(run), offer.name.clone());
        move |row| {
            if let Some(run) = run.upgrade() {
                let mut join = run.join.borrow_mut();
                if row.is_active() {
                    join.stores_off.remove(&name);
                } else {
                    join.stores_off.insert(name.clone());
                }
            }
        }
    });
    row.upcast()
}

fn note(text: &str) -> gtk::Label {
    let label = widgets::haze(text, "caption");
    label.set_xalign(0.0);
    label.set_wrap(true);
    label
}

/// Continue on the paired page: what was left on comes over, then the stores asked answer.
fn carry_on(run: &Rc<Run>) {
    let ctx = &run.ctx;
    let mut join = run.join.borrow_mut();
    if !join.landed() || join.working() {
        return;
    }
    let first = join.presses == 0;
    let setup = join.setup.clone();
    let device_id = setup.as_ref().map(|s| s.device.clone()).or_else(|| {
        let name = join.device.clone()?;
        ctx.hub.known.borrow().sync_group.as_ref().and_then(|s| {
            s.devices.iter().find(|d| !d.this_device && d.name == name).map(|d| d.id.clone())
        })
    });
    if first {
        if let Some(answer) = join.medium.or(setup.as_ref().and_then(|s| s.medium)) {
            run.answer.set(Some(answer));
            medium::first_run_answered(ctx, answer);
        }
        if !join.sign_ins
            && let Some(id) = &device_id
        {
            ctx.hub.send(Command::SyncGroup(SyncCommand::CopySignIns {
                device: Some(id.clone()),
                copy: Vec::new(),
                address: None,
            }));
        }
        if let (Some(setup), Some(id)) = (&setup, &device_id) {
            let servers: Vec<String> = setup
                .servers
                .iter()
                .filter(|s| !s.added && !join.servers_off.contains(&s.id))
                .map(|s| s.id.clone())
                .collect();
            if !servers.is_empty() {
                ctx.hub.send(Command::SyncGroup(SyncCommand::AddBookServersFrom {
                    device: id.clone(),
                    servers,
                }));
            }
        }
    }
    // Stores left on that have not joined yet: a refusal asks again on the next press.
    let refused: HashSet<String> = join.refused.iter().map(|(n, _)| n.clone()).collect();
    let stores: Vec<SyncStoreOffer> = setup
        .as_ref()
        .map(|s| s.stores.clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|o| {
            !join.stores_off.contains(&o.name)
                && (!join.answered.contains(&o.name) || refused.contains(&o.name))
        })
        .collect();
    for offer in &stores {
        join.answered.remove(&offer.name);
    }
    join.refused.retain(|(n, _)| !stores.iter().any(|o| o.name == *n));
    join.asked = stores.iter().map(|o| o.name.clone()).collect();
    join.asked_at = Some(Instant::now());
    join.presses += 1;
    if let Some(id) = device_id
        && !stores.is_empty()
    {
        ctx.hub.send(Command::SyncGroup(SyncCommand::JoinStoresFrom {
            device: Some(id),
            stores: stores.iter().map(|o| o.address.clone()).collect(),
            address: None,
        }));
    }
    drop(join);
    refresh(run);
}

/// The stores answered and none refused: on to the first page still to ask, or Home.
fn leave_paired(run: &Rc<Run>) {
    let left = {
        let mut join = run.join.borrow_mut();
        if join.presses == 0 || join.gone {
            return;
        }
        join.gone = true;
        join.pages_left()
    };
    let next = left.first().map(|page| match page {
        FirstRunPage::Medium => Step::Medium,
        FirstRunPage::Lists => Step::Lists,
        FirstRunPage::Sources => Step::Sources,
    });
    match next.filter(|step| run.steps.contains(step)) {
        Some(step) => push(run, step),
        None => {
            run.dialog.close();
        }
    }
}

fn heard(run: &Rc<Run>, event: &Event) {
    let stage = run.join.borrow().stage;
    match event {
        Event::SyncInvite { .. } if stage == Stage::Code => refresh_code(run),
        Event::SyncStatus(status) if stage == Stage::Code => {
            let started = run.join.borrow().started_with;
            if status.devices.len() > started
                && let Some(other) = status.devices.iter().rev().find(|d| !d.this_device)
            {
                paired(run, Some(other.name.clone()));
            }
        }
        Event::SetupFrom(setup) if stage != Stage::Ask => {
            {
                let mut join = run.join.borrow_mut();
                join.device = Some(setup.device_name.clone());
                join.setup = Some(setup.clone());
                if join.medium.is_none() {
                    join.medium = setup.medium;
                }
            }
            paired(run, Some(setup.device_name.clone()));
            refresh(run);
        }
        Event::SyncSignInsCopied { trackers, .. } if stage != Stage::Ask => {
            run.join.borrow_mut().trackers = trackers.clone();
            refresh(run);
        }
        Event::SyncStoreJoined { name, .. } if stage == Stage::Paired => {
            run.join.borrow_mut().answered.insert(name.clone());
            refresh(run);
        }
        Event::SyncStoreJoinFailed { name, problem, .. } if stage == Stage::Paired => {
            {
                let mut join = run.join.borrow_mut();
                join.answered.insert(name.clone());
                join.refused.push((name.clone(), *problem));
            }
            refresh(run);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// 3 Watch, read, or both?

fn medium_page(run: &Rc<Run>) -> adw::NavigationPage {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let go = medium::continue_button();
    medium::hold_continue(&go, run.answer.get().is_some());
    let list = medium::choices(
        {
            let (run, go) = (Rc::downgrade(run), go.downgrade());
            move |answer| {
                if let Some(run) = run.upgrade() {
                    run.answer.set(Some(answer));
                }
                if let Some(go) = go.upgrade() {
                    medium::hold_continue(&go, true);
                }
            }
        },
        run.answer.get(),
    );
    body.append(&list);
    go.connect_clicked({
        let run = Rc::downgrade(run);
        move |_| {
            let Some(run) = run.upgrade() else { return };
            let Some(answer) = run.answer.get() else { return };
            medium::first_run_answered(&run.ctx, answer);
            go_on(&run, Step::Medium);
        }
    });
    let foot = foot();
    foot.append(&go);
    let (title, line, _) = medium::words(&kaeri_model::medium::MediumAsk::FirstRun);
    let (page, _) = frame("medium", &run.step(Step::Medium), &title, Some(&line), &body, &foot);
    page
}

// ---------------------------------------------------------------------------------------------
// 4 Bring in your lists

fn lists(run: &Rc<Run>) -> adw::NavigationPage {
    let tracker = crate::say::tracker_name("anilist"); // i18n-skip
    let answer = run.answer();
    let (line, later) = match answer {
        KaeriIsFor::Watch => {
            (t::onboarding_lists_line_watch(tracker), t::onboarding_lists_later_watch())
        }
        KaeriIsFor::Read => {
            (t::onboarding_lists_line_read(tracker), t::onboarding_lists_later_read())
        }
        KaeriIsFor::Both => {
            (t::onboarding_lists_line_both(tracker), t::onboarding_lists_later_both())
        }
    };
    let body = gtk::Box::new(gtk::Orientation::Vertical, 10);
    let list = widgets::boxed_list();
    let row = nav_row(
        "avatar-default-symbolic",
        &t::onboarding_connect(tracker),
        &t::onboarding_lists_anilist_line(),
    );
    row.connect_activated({
        let window = run.ctx.window.clone();
        move |_| {
            let _ = WidgetExt::activate_action(
                &window,
                "win.connect",
                Some(&"anilist".to_variant()),
            );
        }
    });
    list.append(&row);
    body.append(&list);
    body.append(&note(&later));
    let foot = foot();
    let next = amber(&t::onboarding_continue());
    next.connect_clicked({
        let run = Rc::downgrade(run);
        move |_| {
            if let Some(run) = run.upgrade() {
                go_on(&run, Step::Lists);
            }
        }
    });
    foot.append(&next);
    let (page, _) = frame(
        "lists",
        &run.step(Step::Lists),
        &t::onboarding_lists_title(),
        Some(&line),
        &body,
        &foot,
    );
    page.connect_shown(|_| came_in::show_in(None));
    page
}

// ---------------------------------------------------------------------------------------------
// 5 Where the shows or books come from

fn sources(run: &Rc<Run>) -> adw::NavigationPage {
    let answer = run.answer();
    let ctx = &run.ctx;
    let body = gtk::Box::new(gtk::Orientation::Vertical, 18);
    // What was added from here, each with a check, over the rest.
    let added = widgets::boxed_list();
    added.set_visible(false);
    body.append(&added);
    run.added_list.replace(Some(added));
    draw_added(run);
    let done = {
        let dialog = run.dialog.downgrade();
        move || {
            if let Some(dialog) = dialog.upgrade() {
                dialog.close();
            }
        }
    };
    body.append(
        &crate::reading::demo::try_card(ctx, answer == KaeriIsFor::Both, done.clone())
            .upcast::<gtk::Widget>(),
    );
    let shows = |list: &gtk::ListBox| {
        let folder = nav_row(
            "folder-symbolic",
            &t::onboarding_books_folder(),
            &t::onboarding_both_shows_folder_line(&crate::sync::kind()),
        );
        folder.connect_activated({
            let ctx = Rc::downgrade(ctx);
            move |_| {
                if let Some(ctx) = ctx.upgrade() {
                    crate::pages::folders::choose(&ctx);
                }
            }
        });
        list.append(&folder);
        let server = nav_row(
            "network-server-symbolic",
            &t::onboarding_books_server(),
            &t::onboarding_both_shows_server_line(),
        );
        server.connect_activated({
            let ctx = Rc::downgrade(ctx);
            move |_| {
                if let Some(ctx) = ctx.upgrade() {
                    crate::pages::servers::open_add(&ctx, None);
                }
            }
        });
        list.append(&server);
    };
    let books = |list: &gtk::ListBox| {
        let folder = nav_row(
            "folder-symbolic",
            &t::onboarding_books_folder(),
            &t::onboarding_books_folder_line(),
        );
        folder.connect_activated({
            let ctx = Rc::downgrade(ctx);
            move |_| {
                if let Some(ctx) = ctx.upgrade() {
                    came_in::add_folder(&ctx);
                }
            }
        });
        list.append(&folder);
        let server = nav_row(
            "network-server-symbolic",
            &t::onboarding_books_server(),
            &t::onboarding_books_server_line(),
        );
        server.connect_activated({
            let ctx = Rc::downgrade(ctx);
            move |_| {
                if let Some(ctx) = ctx.upgrade() {
                    came_in::add_server(&ctx, None);
                }
            }
        });
        list.append(&server);
    };
    let (title, line) = match answer {
        KaeriIsFor::Watch => {
            let list = widgets::boxed_list();
            shows(&list);
            body.append(&list);
            (t::onboarding_sources_title(), t::onboarding_where_hosts_linux())
        }
        KaeriIsFor::Read => {
            let list = widgets::boxed_list();
            books(&list);
            body.append(&list);
            (t::onboarding_books_title(), t::onboarding_books_line())
        }
        KaeriIsFor::Both => {
            for (heading, fill) in [
                (t::onboarding_both_shows(), &shows as &dyn Fn(&gtk::ListBox)),
                (t::onboarding_both_books(), &books),
            ] {
                let group = gtk::Box::new(gtk::Orientation::Vertical, 8);
                group.append(&widgets::heading(&heading));
                let list = widgets::boxed_list();
                fill(&list);
                group.append(&list);
                body.append(&group);
            }
            (t::onboarding_both_title(), t::onboarding_both_line())
        }
    };
    // A plugin store invite: pasting one opens the store's page here.
    let invites = widgets::boxed_list();
    let invite = adw::EntryRow::builder().title(t::onboarding_books_invite_heading()).build();
    invite.set_show_apply_button(true);
    invite.set_input_purpose(gtk::InputPurpose::Url);
    invite.connect_apply({
        let ctx = Rc::downgrade(ctx);
        move |row| {
            let text = row.text().trim().to_owned();
            if let Some(ctx) = ctx.upgrade()
                && !text.is_empty()
            {
                row.set_text("");
                came_in::join_store(&ctx, Some(&text));
            }
        }
    });
    invites.append(&invite);
    body.append(&invites);
    let foot = foot();
    let finish = amber(&t::action_done());
    finish.connect_clicked(move |_| done());
    foot.append(&finish);
    let step = run.step(Step::Sources);
    let (page, _) = frame("sources", &step, &title, Some(&line), &body, &foot);
    came_in::show_in(Some((run.pages.clone(), step.clone())));
    page.connect_shown({
        let (pages, step) = (run.pages.clone(), step);
        move |_| came_in::show_in(Some((pages.clone(), step.clone())))
    });
    page
}

/// The sources added from first run, each with its check, over the add rows.
fn draw_added(run: &Rc<Run>) {
    let Some(list) = run.added_list.borrow().clone() else { return };
    list.remove_all();
    for added in run.added.borrow().iter() {
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&added.title))
            .subtitle(glib::markup_escape_text(&added.line))
            .build();
        row.add_prefix(&came_in::tile(added.icon));
        let check = gtk::Image::from_icon_name("object-select-symbolic");
        check.set_accessible_role(gtk::AccessibleRole::Presentation);
        row.add_suffix(&check);
        list.append(&row);
    }
    let any = list.first_child().is_some();
    if any && !list.is_visible() && !crate::motion::reduced() {
        crate::motion::crossfade_in(&list);
    }
    list.set_visible(any);
}

// ---------------------------------------------------------------------------------------------
// Shots

/// For a shot: first run on `step` ("join", "code", "medium", "sources"), with reading's answer.
#[cfg(feature = "fixtures")]
pub fn shot(ctx: &Rc<Ctx>, step: &str, answer: Option<KaeriIsFor>) -> Option<()> {
    open(ctx);
    let run = shown_run(ctx)?;
    run.answer.set(answer);
    match step {
        "join" => push(&run, Step::Join),
        "medium" => push(&run, Step::Medium),
        "sources" => push(&run, Step::Sources),
        _ => {}
    }
    Some(())
}

/// For a shot: the join page's code, as it stands `seconds` after it showed, with `left` seconds
/// on it.
#[cfg(feature = "fixtures")]
pub fn shot_code(ctx: &Rc<Ctx>, code: &str, seconds: u64, left: i64) -> Option<()> {
    open(ctx);
    let run = shown_run(ctx)?;
    push(&run, Step::Join);
    {
        let mut join = run.join.borrow_mut();
        join.stage = Stage::Code;
        join.code_at = Instant::now().checked_sub(Duration::from_secs(seconds));
        join.started_with = usize::MAX;
    }
    ctx.hub.known.borrow_mut().invite =
        Some((code.to_owned(), 48_213, glib::real_time() / 1_000_000 + left));
    run.pages.push(&code_page(&run));
    start_tick(&run);
    Some(())
}

/// For a shot: paired with `device`, with what it brought, or still arriving, and the stores
/// that refused.
#[cfg(feature = "fixtures")]
pub fn shot_paired(
    ctx: &Rc<Ctx>,
    device: &str,
    trackers: Vec<String>,
    setup: Option<JoinedSetup>,
    refused: Vec<String>,
) -> Option<()> {
    open(ctx);
    let run = shown_run(ctx)?;
    {
        let mut join = run.join.borrow_mut();
        join.stage = Stage::Code;
        join.sign_ins = true;
    }
    paired(&run, Some(device.to_owned()));
    {
        let mut join = run.join.borrow_mut();
        join.paired_at = Instant::now().checked_sub(Duration::from_secs(1));
        join.trackers = trackers;
        join.medium = setup.as_ref().and_then(|s| s.medium);
        join.setup = setup;
        for name in refused {
            join.answered.insert(name.clone());
            join.refused.push((name, SyncStoreProblem::Refused));
        }
    }
    refresh(&run);
    Some(())
}

/// For a shot: an add came in on first run's sources page.
#[cfg(feature = "fixtures")]
pub fn shot_added(ctx: &Rc<Ctx>, added: came_in::Added) -> Option<()> {
    let run = shown_run(ctx)?;
    run.added.borrow_mut().push(added);
    draw_added(&run);
    Some(())
}
