//! Whether a book server took the progress read here: the line on the page between chapters
//! ("Saved to books.home.example") and the line on a server's row in Your sources ("Progress
//! saved 2 minutes ago"). Both keep the waiting rule: "Sending" shows only after 600 ms, and
//! once it has, for at least 300 ms. Try again dims until the server answers.

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
};

use adw::prelude::*;
use kaeri_core::books::{ChapterId, WorkId};
use kaeri_i18n::t;
use kaeri_model::{
    book_sources::{
        BookServerKind, BookServerRow, BookSourceCommand, BookSourceEvent, WriteBackProblem,
        WriteBackResult,
    },
    command::Command,
    event::Event,
    reader::ReaderEvent,
};

use super::came_in::Wait;
use crate::{hub::Hub, window::Ctx};

/// Why progress was not saved, when trying again will not help by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhyNot {
    Key,
    Password,
    Gone,
    GaveUp,
}

fn why_not(problem: WriteBackProblem) -> Option<WhyNot> {
    match problem {
        WriteBackProblem::RefusedKey => Some(WhyNot::Key),
        WriteBackProblem::RefusedPassword => Some(WhyNot::Password),
        WriteBackProblem::SeriesGone => Some(WhyNot::Gone),
        WriteBackProblem::NoAnswerForADay => Some(WhyNot::GaveUp),
        WriteBackProblem::Offline
        | WriteBackProblem::NoAnswer
        | WriteBackProblem::ServerError
        | WriteBackProblem::NotSetToReceive => None,
    }
}

fn reason(why: WhyNot) -> String {
    match why {
        WhyNot::Key => t::progress_why_key(),
        WhyNot::Password => t::progress_why_password(),
        WhyNot::Gone => t::progress_why_gone(),
        WhyNot::GaveUp => t::progress_why_gave_up(),
    }
}

/// The line on the page between chapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerLine {
    Saved(String),
    Sending(String),
    Offline(String),
    Retrying(String),
    Failed(String, WhyNot),
}

impl ServerLine {
    #[cfg_attr(not(feature = "fixtures"), allow(dead_code))]
    pub fn can_retry(&self) -> bool {
        matches!(self, Self::Retrying(_) | Self::Failed(..))
    }

    fn of_result(result: &WriteBackResult, host: &str) -> Option<Self> {
        match result {
            WriteBackResult::Saved => Some(Self::Saved(host.to_owned())),
            WriteBackResult::Waiting { problem } | WriteBackResult::NotSaved { problem } => {
                Self::of_problem(*problem, host)
            }
        }
    }

    fn of_problem(problem: WriteBackProblem, host: &str) -> Option<Self> {
        match problem {
            WriteBackProblem::NotSetToReceive => None,
            WriteBackProblem::Offline => Some(Self::Offline(host.to_owned())),
            other => Some(why_not(other).map_or_else(
                || Self::Retrying(host.to_owned()),
                |why| Self::Failed(host.to_owned(), why),
            )),
        }
    }

    #[cfg_attr(not(feature = "fixtures"), allow(dead_code))]
    pub fn words(&self) -> String {
        match self {
            Self::Saved(host) => t::progress_server_saved(host),
            Self::Sending(host) => t::progress_server_sending(host),
            Self::Offline(host) => t::progress_server_offline(host),
            Self::Retrying(host) => t::progress_server_retrying(host),
            Self::Failed(host, why) => t::progress_server_failed(host, &reason(*why)),
        }
    }
}

/// The line on a server's row in Your sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowLine {
    Off,
    Saved(i64),
    Sending(u32),
    Offline(u32),
    Retrying(u32),
    Failed(u32, WhyNot),
}

impl RowLine {
    pub fn can_retry(&self) -> bool {
        matches!(self, Self::Retrying(_) | Self::Failed(..))
    }

    pub fn words(&self) -> String {
        match self {
            Self::Off => t::progress_row_off(&t::device_kind_computer()),
            Self::Saved(at) => t::progress_row_saved(&super::ago(*at)),
            Self::Sending(n) => t::progress_row_sending(i64::from(*n)),
            Self::Offline(n) => t::progress_row_offline(i64::from(*n)),
            Self::Retrying(n) => t::progress_row_retrying(i64::from(*n)),
            Self::Failed(n, why) => t::progress_row_failed(i64::from(*n), &reason(*why)),
        }
    }
}

/// Where a server's progress stands, from its row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stand {
    None,
    Off,
    Going(u32),
    Saved(i64),
    Offline(u32),
    Retrying(u32),
    Failed(u32, WhyNot),
}

impl Stand {
    fn of(row: Option<&BookServerRow>) -> Self {
        let Some(row) = row.filter(|r| r.kind != BookServerKind::Opds) else {
            return Self::None;
        };
        if !row.report_progress
            || row.progress_problem == Some(WriteBackProblem::NotSetToReceive)
        {
            return Self::Off;
        }
        let count = row.progress_pending;
        if count == 0 {
            return row.progress_saved_at.map_or(Self::None, Self::Saved);
        }
        match row.progress_problem {
            None => Self::Going(count),
            Some(WriteBackProblem::Offline) => Self::Offline(count),
            Some(problem) => {
                why_not(problem).map_or(Self::Retrying(count), |why| Self::Failed(count, why))
            }
        }
    }

    fn row_line(self) -> Option<RowLine> {
        match self {
            Self::None | Self::Going(_) => None,
            Self::Off => Some(RowLine::Off),
            Self::Saved(at) => Some(RowLine::Saved(at)),
            Self::Offline(n) => Some(RowLine::Offline(n)),
            Self::Retrying(n) => Some(RowLine::Retrying(n)),
            Self::Failed(n, why) => Some(RowLine::Failed(n, why)),
        }
    }
}

fn stamp(row: Option<&BookServerRow>) -> (Option<i64>, Option<i64>) {
    row.map_or((None, None), |r| (r.progress_saved_at, r.progress_problem_at))
}

/// A press of Try again, until the server's row says it was answered.
struct Press {
    at: (Option<i64>, Option<i64>),
    sent: Cell<bool>,
}

impl Press {
    fn answered(&self, row: Option<&BookServerRow>) -> bool {
        let Some(row) = row else { return true };
        if row.progress_sending {
            self.sent.set(true);
            return false;
        }
        match Stand::of(Some(row)) {
            Stand::Offline(_) | Stand::Off | Stand::None | Stand::Saved(_) => true,
            _ => self.sent.get() || stamp(Some(row)) != self.at,
        }
    }
}

/// The lines, as the engine says them. One for the window, made as it opens.
#[derive(Default)]
pub struct Model {
    hub: RefCell<std::rc::Weak<Hub>>,
    /// The page-between-chapters line, by chapter.
    lines: RefCell<HashMap<ChapterId, ServerLine>>,
    /// Each server row's line, by server id.
    row_lines: RefCell<HashMap<String, RowLine>>,
    /// Servers whose Try again waits for an answer, dimmed.
    pub retrying: RefCell<HashSet<String>>,
    waits: RefCell<HashMap<ChapterId, Wait>>,
    servers: RefCell<HashMap<ChapterId, String>>,
    hosts: RefCell<HashMap<ChapterId, String>>,
    answered: RefCell<HashSet<ChapterId>>,
    follows: RefCell<HashSet<ChapterId>>,
    line_pressed: RefCell<HashMap<String, Press>>,
    rows: RefCell<HashMap<String, BookServerRow>>,
    row_waits: RefCell<HashMap<String, Wait>>,
    row_pressed: RefCell<HashMap<String, Press>>,
    counts: RefCell<HashMap<String, u32>>,
    /// The host each work's progress goes to, as the reader last opened it.
    progress_hosts: RefCell<HashMap<WorkId, String>>,
    /// Who hears a line change: the end page and Your sources.
    heard: RefCell<Vec<(u64, Listener)>>,
    next: Cell<u64>,
}

/// One who hears a line change.
type Listener = Rc<dyn Fn()>;

thread_local! {
    static MODEL: RefCell<Option<Rc<Model>>> = const { RefCell::new(None) };
}

/// The window's model, made and listening from the first call on.
pub fn model(ctx: &Rc<Ctx>) -> Rc<Model> {
    if let Some(model) = MODEL.with(|m| m.borrow().clone()) {
        return model;
    }
    let model = Rc::new(Model::default());
    model.hub.replace(Rc::downgrade(&ctx.hub));
    MODEL.with(|m| m.replace(Some(Rc::clone(&model))));
    let weak = Rc::downgrade(&model);
    ctx.hub.listen(move |event| {
        if let Some(model) = weak.upgrade() {
            model.apply(event);
        }
    });
    if let Some(view) = ctx.hub.known.borrow().book_sources.as_ref() {
        model.update(&view.servers);
    }
    model
}

impl Model {
    /// Hear each change of a line; the number is for [`Model::forget`].
    pub fn on_change(&self, heard: impl Fn() + 'static) -> u64 {
        let id = self.next.get() + 1;
        self.next.set(id);
        self.heard.borrow_mut().push((id, Rc::new(heard)));
        id
    }

    pub fn forget(&self, id: u64) {
        self.heard.borrow_mut().retain(|(at, _)| *at != id);
    }

    fn changed(&self) {
        let heard: Vec<_> = self.heard.borrow().iter().map(|(_, h)| Rc::clone(h)).collect();
        for heard in heard {
            heard();
        }
    }

    #[cfg_attr(not(feature = "fixtures"), allow(dead_code))]
    pub fn line(&self, chapter: &ChapterId) -> Option<ServerLine> {
        self.lines.borrow().get(chapter).cloned()
    }

    pub fn row_line(&self, server: &str) -> Option<RowLine> {
        self.row_lines.borrow().get(server).cloned()
    }

    fn apply(self: &Rc<Self>, event: &Event) {
        match event {
            Event::Reader(ReaderEvent::Opened(view)) => {
                let mut hosts = self.progress_hosts.borrow_mut();
                match &view.progress_host {
                    Some(host) => hosts.insert(view.work.clone(), host.clone()),
                    None => hosts.remove(&view.work),
                };
            }
            Event::Reader(ReaderEvent::ChapterRead { work, chapter, .. }) => {
                self.chapter_read(work, chapter);
            }
            Event::Reader(ReaderEvent::ProgressSent {
                chapter, server, host, result, ..
            }) => {
                self.sent(chapter, server, host, result);
            }
            Event::BookSources(BookSourceEvent::View(view)) => self.update(&view.servers),
            _ => {}
        }
    }

    /// Whether the page between chapters of `work` keeps room for the line.
    /// The reader's end page asks it (linux/reading-polish).
    #[allow(dead_code)]
    pub fn holds(&self, work: &WorkId, chapter: &ChapterId) -> bool {
        self.lines.borrow().contains_key(chapter)
            || self.waits.borrow().contains_key(chapter)
            || self.progress_hosts.borrow().contains_key(work)
    }

    fn chapter_read(self: &Rc<Self>, work: &WorkId, chapter: &ChapterId) {
        if self.answered.borrow().contains(chapter) || self.waits.borrow().contains_key(chapter)
        {
            return;
        }
        let Some(host) = self.progress_hosts.borrow().get(work).cloned() else { return };
        self.hosts.borrow_mut().insert(chapter.clone(), host);
        self.wait(chapter);
    }

    fn sent(
        self: &Rc<Self>,
        chapter: &ChapterId,
        server: &str,
        host: &str,
        result: &WriteBackResult,
    ) {
        self.answered.borrow_mut().insert(chapter.clone());
        self.servers.borrow_mut().insert(chapter.clone(), server.to_owned());
        self.hosts.borrow_mut().insert(chapter.clone(), host.to_owned());
        let said = ServerLine::of_result(result, host);
        if matches!(said, None | Some(ServerLine::Saved(_))) {
            self.follows.borrow_mut().remove(chapter);
        } else {
            self.follows.borrow_mut().insert(chapter.clone());
        }
        self.show(chapter, said);
    }

    /// Try again on the line between chapters.
    #[cfg_attr(not(feature = "fixtures"), allow(dead_code))]
    pub fn retry(self: &Rc<Self>, chapter: &ChapterId) {
        let Some(server) = self.servers.borrow().get(chapter).cloned() else { return };
        if !self.lines.borrow().get(chapter).is_some_and(ServerLine::can_retry)
            || self.line_pressed.borrow().contains_key(&server)
        {
            return;
        }
        let at = stamp(self.rows.borrow().get(&server));
        self.line_pressed
            .borrow_mut()
            .insert(server.clone(), Press { at, sent: Cell::new(false) });
        let others: Vec<ChapterId> = self
            .follows
            .borrow()
            .iter()
            .filter(|c| {
                self.servers.borrow().get(*c) == Some(&server)
                    && !self.waits.borrow().contains_key(*c)
            })
            .cloned()
            .collect();
        for other in others {
            self.wait(&other);
        }
        self.send(BookSourceCommand::RetryProgress { id: server });
    }

    fn send(&self, command: BookSourceCommand) {
        if let Some(hub) = self.hub.borrow().upgrade() {
            hub.send(Command::BookSources(command));
        }
    }

    fn follow_rows(self: &Rc<Self>) {
        {
            let rows = self.rows.borrow();
            self.line_pressed
                .borrow_mut()
                .retain(|server, press| !press.answered(rows.get(server)));
        }
        let follows: Vec<ChapterId> = self.follows.borrow().iter().cloned().collect();
        for chapter in follows {
            let Some(server) = self.servers.borrow().get(&chapter).cloned() else { continue };
            if self.line_pressed.borrow().contains_key(&server) {
                continue;
            }
            let Some(host) = self.hosts.borrow().get(&chapter).cloned() else { continue };
            let stand = Stand::of(self.rows.borrow().get(&server));
            match stand {
                Stand::Going(_) => {
                    if !self.waits.borrow().contains_key(&chapter) {
                        self.wait(&chapter);
                    }
                }
                Stand::Saved(_) => self.settle(&chapter, Some(ServerLine::Saved(host))),
                Stand::Offline(_) => self.settle(&chapter, Some(ServerLine::Offline(host))),
                Stand::Retrying(_) => self.settle(&chapter, Some(ServerLine::Retrying(host))),
                Stand::Failed(_, why) => {
                    self.settle(&chapter, Some(ServerLine::Failed(host, why)))
                }
                Stand::Off | Stand::None => self.settle(&chapter, None),
            }
        }
    }

    fn settle(self: &Rc<Self>, chapter: &ChapterId, line: Option<ServerLine>) {
        if matches!(line, None | Some(ServerLine::Saved(_))) {
            self.follows.borrow_mut().remove(chapter);
        }
        if !self.waits.borrow().contains_key(chapter)
            && self.lines.borrow().get(chapter) == line.as_ref()
        {
            return;
        }
        self.show(chapter, line);
    }

    fn wait(self: &Rc<Self>, chapter: &ChapterId) {
        if let Some(old) = self.waits.borrow_mut().remove(chapter) {
            old.cancel();
        }
        let wait = Wait::default();
        let (weak, at) = (Rc::downgrade(self), chapter.clone());
        wait.start(move || {
            let Some(model) = weak.upgrade() else { return };
            let Some(host) = model.hosts.borrow().get(&at).cloned() else { return };
            model.lines.borrow_mut().insert(at.clone(), ServerLine::Sending(host));
            model.changed();
        });
        self.waits.borrow_mut().insert(chapter.clone(), wait);
    }

    fn show(self: &Rc<Self>, chapter: &ChapterId, line: Option<ServerLine>) {
        let (weak, at) = (Rc::downgrade(self), chapter.clone());
        let put = move || {
            let Some(model) = weak.upgrade() else { return };
            match line {
                Some(line) => model.lines.borrow_mut().insert(at, line),
                None => model.lines.borrow_mut().remove(&at),
            };
            model.changed();
        };
        let wait = self.waits.borrow_mut().remove(chapter);
        match wait {
            Some(wait) => wait.end(put),
            None => put(),
        }
    }

    /// Forget the lines between chapters, as the reader closes.
    /// The reader's end page calls it (linux/reading-polish).
    #[allow(dead_code)]
    pub fn clear_lines(&self) {
        for wait in self.waits.borrow_mut().drain().map(|(_, w)| w) {
            wait.cancel();
        }
        self.servers.borrow_mut().clear();
        self.hosts.borrow_mut().clear();
        self.answered.borrow_mut().clear();
        self.follows.borrow_mut().clear();
        self.line_pressed.borrow_mut().clear();
        self.lines.borrow_mut().clear();
    }

    fn update(self: &Rc<Self>, now: &[BookServerRow]) {
        let ids: HashSet<&str> = now.iter().map(|r| r.id.as_str()).collect();
        let gone: Vec<String> =
            self.rows.borrow().keys().filter(|k| !ids.contains(k.as_str())).cloned().collect();
        for id in gone {
            self.forget_row(&id);
        }
        for row in now {
            self.rows.borrow_mut().insert(row.id.clone(), row.clone());
            self.one(&row.id);
        }
        self.follow_rows();
        self.changed();
    }

    fn one(self: &Rc<Self>, id: &str) {
        let row = self.rows.borrow().get(id).cloned();
        let answered = self.row_pressed.borrow().get(id).map(|p| p.answered(row.as_ref()));
        match answered {
            Some(false) => {
                self.going(id, row.as_ref().map_or(0, |r| r.progress_pending));
                return;
            }
            Some(true) => {
                self.row_pressed.borrow_mut().remove(id);
            }
            None => {}
        }
        match Stand::of(row.as_ref()) {
            Stand::Going(count) => self.going(id, count),
            stand => self.settle_row(id, stand.row_line()),
        }
    }

    fn going(self: &Rc<Self>, id: &str, count: u32) {
        if count > 0 {
            self.counts.borrow_mut().insert(id.to_owned(), count);
        }
        let showing = self.row_waits.borrow().get(id).map(Wait::showing);
        match showing {
            Some(true) => {
                let n = self.counts.borrow().get(id).copied().unwrap_or(1);
                self.row_lines.borrow_mut().insert(id.to_owned(), RowLine::Sending(n));
            }
            Some(false) => {}
            None => {
                let wait = Wait::default();
                let (weak, at) = (Rc::downgrade(self), id.to_owned());
                wait.start(move || {
                    let Some(model) = weak.upgrade() else { return };
                    let n = model.counts.borrow().get(&at).copied().unwrap_or(1);
                    model.row_lines.borrow_mut().insert(at.clone(), RowLine::Sending(n));
                    model.changed();
                });
                self.row_waits.borrow_mut().insert(id.to_owned(), wait);
            }
        }
    }

    fn settle_row(self: &Rc<Self>, id: &str, line: Option<RowLine>) {
        let (weak, at) = (Rc::downgrade(self), id.to_owned());
        let put = move || {
            let Some(model) = weak.upgrade() else { return };
            model.retrying.borrow_mut().remove(&at);
            match line {
                Some(line) => model.row_lines.borrow_mut().insert(at, line),
                None => model.row_lines.borrow_mut().remove(&at),
            };
            model.changed();
        };
        let wait = self.row_waits.borrow_mut().remove(id);
        match wait {
            Some(wait) => wait.end(put),
            None => put(),
        }
    }

    /// Try again on a server's row: dimmed until the server answers.
    pub fn retry_row(self: &Rc<Self>, id: &str) {
        if self.row_pressed.borrow().contains_key(id)
            || !self.row_lines.borrow().get(id).is_some_and(RowLine::can_retry)
        {
            return;
        }
        let at = stamp(self.rows.borrow().get(id));
        self.row_pressed
            .borrow_mut()
            .insert(id.to_owned(), Press { at, sent: Cell::new(false) });
        self.retrying.borrow_mut().insert(id.to_owned());
        self.send(BookSourceCommand::RetryProgress { id: id.to_owned() });
        self.one(id);
        self.changed();
    }

    fn forget_row(&self, id: &str) {
        if let Some(wait) = self.row_waits.borrow_mut().remove(id) {
            wait.cancel();
        }
        self.rows.borrow_mut().remove(id);
        self.row_pressed.borrow_mut().remove(id);
        self.counts.borrow_mut().remove(id);
        self.retrying.borrow_mut().remove(id);
        self.row_lines.borrow_mut().remove(id);
    }

    /// Set a line as it stands, for the shots.
    #[cfg(feature = "fixtures")]
    pub fn stage_line(&self, chapter: &ChapterId, server: &str, line: Option<ServerLine>) {
        self.servers.borrow_mut().insert(chapter.clone(), server.to_owned());
        self.answered.borrow_mut().insert(chapter.clone());
        match line {
            Some(line) => self.lines.borrow_mut().insert(chapter.clone(), line),
            None => self.lines.borrow_mut().remove(chapter),
        };
        self.changed();
    }

    /// Set a row's line as it stands, for the shots.
    #[cfg(feature = "fixtures")]
    pub fn stage_row(&self, id: &str, line: Option<RowLine>, pressing: bool) {
        match line {
            Some(line) => self.row_lines.borrow_mut().insert(id.to_owned(), line),
            None => self.row_lines.borrow_mut().remove(id),
        };
        if pressing {
            self.retrying.borrow_mut().insert(id.to_owned());
        } else {
            self.retrying.borrow_mut().remove(id);
        }
        self.changed();
    }
}

// ---------------------------------------------------------------------------------------------
// The widgets

/// The line on the page between chapters: the words, and Try again as a link under a line that
/// failed. It keeps its room from the start when the work's server takes progress, so nothing
/// moves when the line comes.
#[cfg_attr(not(feature = "fixtures"), allow(dead_code))]
pub struct EndLine {
    pub widget: gtk::Box,
    words: gtk::Label,
    /// The error glyph before a line that failed, and the small spinner before one being sent.
    failed: gtk::Image,
    sending: adw::Spinner,
    retry: gtk::Button,
}

#[cfg_attr(not(feature = "fixtures"), allow(dead_code))]
impl EndLine {
    /// The line for `chapter`, following the model while the widget lives.
    pub fn new(ctx: &Rc<Ctx>, chapter: &ChapterId) -> Rc<Self> {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 2);
        widget.add_css_class("server-line");
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let failed = gtk::Image::from_icon_name("dialog-warning-symbolic");
        failed.add_css_class("came-in-failed");
        failed.set_pixel_size(12);
        failed.set_valign(gtk::Align::Center);
        failed.set_visible(false);
        row.append(&failed);
        let sending = adw::Spinner::new();
        sending.set_size_request(12, 12);
        sending.set_valign(gtk::Align::Center);
        sending.set_visible(false);
        row.append(&sending);
        let words = gtk::Label::new(None);
        words.add_css_class("haze");
        words.set_xalign(0.0);
        words.set_wrap(true);
        row.append(&words);
        widget.append(&row);
        // A neutral link: the page's one amber is Continue.
        let retry = gtk::Button::with_label(&t::action_try_again());
        retry.add_css_class("link");
        retry.add_css_class("flat");
        retry.add_css_class("plain-link");
        retry.set_halign(gtk::Align::Start);
        retry.set_visible(false);
        widget.append(&retry);
        let line = Rc::new(Self { widget, words, failed, sending, retry });
        let model = model(ctx);
        line.retry.connect_clicked({
            let (model, chapter) = (Rc::downgrade(&model), chapter.clone());
            move |button| {
                if let Some(model) = model.upgrade() {
                    button.set_opacity(0.38);
                    model.retry(&chapter);
                }
            }
        });
        let id = model.on_change({
            let (weak, model_weak, chapter) =
                (Rc::downgrade(&line), Rc::downgrade(&model), chapter.clone());
            move || {
                if let (Some(line), Some(model)) = (weak.upgrade(), model_weak.upgrade()) {
                    line.set(model.line(&chapter).as_ref());
                }
            }
        });
        let weak_model = Rc::downgrade(&model);
        line.widget.connect_destroy(move |_| {
            if let Some(model) = weak_model.upgrade() {
                model.forget(id);
            }
        });
        line.set(model.line(chapter).as_ref());
        line
    }

    fn set(&self, line: Option<&ServerLine>) {
        let text = line.map(ServerLine::words).unwrap_or_default();
        if self.words.label() != text {
            self.words.set_label(&text);
        }
        self.failed.set_visible(matches!(line, Some(ServerLine::Failed(..))));
        self.sending.set_visible(matches!(line, Some(ServerLine::Sending(_))));
        let retry = line.is_some_and(ServerLine::can_retry);
        self.retry.set_visible(retry);
        if !retry {
            self.retry.set_opacity(1.0);
        }
    }
}

/// The row line's words and its Try again, for a server row in Your sources. `retry` is put in
/// the row's suffix in place of the chevron while the line can be retried.
pub fn row_parts(model: &Rc<Model>, id: &str) -> (Option<String>, Option<gtk::Button>) {
    let line = model.row_line(id);
    let words = line.as_ref().map(RowLine::words);
    let retry = line.filter(RowLine::can_retry).map(|_| {
        let button = gtk::Button::with_label(&t::action_try_again());
        button.set_valign(gtk::Align::Center);
        if model.retrying.borrow().contains(id) {
            button.set_opacity(0.38);
            button.set_can_target(false);
        }
        let (model, id) = (Rc::downgrade(model), id.to_owned());
        button.connect_clicked(move |button| {
            if let Some(model) = model.upgrade() {
                button.set_opacity(0.38);
                button.set_can_target(false);
                model.retry_row(&id);
            }
        });
        button
    });
    (words, retry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(pending: u32, problem: Option<WriteBackProblem>) -> BookServerRow {
        BookServerRow {
            id: "s".into(),
            kind: BookServerKind::Kavita,
            name: "Books".into(),
            address: "https://books.home.example".into(),
            series: 1,
            report_progress: true,
            off: false,
            signed_out: false,
            libraries: 1,
            empty: false,
            rechecks_at: None,
            key_last4: None,
            progress_pending: pending,
            progress_sending: false,
            progress_saved_at: Some(10),
            progress_problem: problem,
            progress_problem_at: None,
        }
    }

    #[test]
    fn a_rows_stand_follows_its_progress() {
        assert_eq!(Stand::of(Some(&row(0, None))), Stand::Saved(10));
        assert_eq!(Stand::of(Some(&row(2, None))), Stand::Going(2));
        assert_eq!(
            Stand::of(Some(&row(2, Some(WriteBackProblem::Offline)))),
            Stand::Offline(2)
        );
        assert_eq!(
            Stand::of(Some(&row(2, Some(WriteBackProblem::NoAnswer)))),
            Stand::Retrying(2)
        );
        assert_eq!(
            Stand::of(Some(&row(1, Some(WriteBackProblem::RefusedKey)))),
            Stand::Failed(1, WhyNot::Key)
        );
        assert_eq!(
            Stand::of(Some(&row(1, Some(WriteBackProblem::NotSetToReceive)))),
            Stand::Off
        );
        let mut opds = row(1, None);
        opds.kind = BookServerKind::Opds;
        assert_eq!(Stand::of(Some(&opds)), Stand::None);
        let mut off = row(0, None);
        off.report_progress = false;
        assert_eq!(Stand::of(Some(&off)), Stand::Off);
    }

    #[test]
    fn a_result_says_its_line() {
        let host = "books.home.example";
        assert_eq!(
            ServerLine::of_result(&WriteBackResult::Saved, host),
            Some(ServerLine::Saved(host.into()))
        );
        assert_eq!(
            ServerLine::of_result(
                &WriteBackResult::Waiting { problem: WriteBackProblem::ServerError },
                host
            ),
            Some(ServerLine::Retrying(host.into()))
        );
        assert_eq!(
            ServerLine::of_result(
                &WriteBackResult::NotSaved { problem: WriteBackProblem::SeriesGone },
                host
            ),
            Some(ServerLine::Failed(host.into(), WhyNot::Gone))
        );
        assert_eq!(
            ServerLine::of_result(
                &WriteBackResult::NotSaved { problem: WriteBackProblem::NotSetToReceive },
                host
            ),
            None
        );
    }

    #[test]
    fn try_again_is_answered_when_the_row_moves() {
        let press = Press { at: (Some(10), None), sent: Cell::new(false) };
        let mut waiting = row(2, Some(WriteBackProblem::NoAnswer));
        assert!(!press.answered(Some(&waiting)));
        waiting.progress_sending = true;
        assert!(!press.answered(Some(&waiting)));
        waiting.progress_sending = false;
        assert!(press.answered(Some(&waiting)));
        assert!(press.answered(None));
    }
}
