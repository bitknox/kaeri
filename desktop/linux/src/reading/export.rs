//! Send to e-reader, as the app board draws it: the e-readers under For, each with the format
//! and screen it gets, the chapters under What, the line that says progress can't come back,
//! and Prepare files. The engine makes the files; this saves them to a folder off the main
//! thread and says where they went.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use gtk::{gio, glib};
use kaeri_core::{
    books::{ChapterId, WorkId},
    media::Medium,
};
use kaeri_i18n::t;
use kaeri_model::{
    event::Event,
    jobs::JobId,
    reading::{
        ChapterReadState, DownloadPick, ExportFile, ExportFormat, ExportTarget, ReadingAbout,
        ReadingCommand, ReadingEvent, VolumeGroup,
    },
};

use super::{listen, send};
use crate::{widgets, window::Ctx};

/// How many unread chapters the first choice sends.
const NEXT: usize = 10;

/// Where the dialog is.
#[derive(Clone, Debug, PartialEq)]
enum Phase {
    Choosing,
    Preparing {
        done: u32,
        total: u32,
    },
    Waiting(String),
    Failed(String),
    Ready(Vec<ExportFile>),
    /// The send was ready and its files are gone: the engine keeps them for a day, and a send
    /// put away loses them at once. Send again prepares them anew.
    Gone,
}

/// The ready files to save, or [`Phase::Gone`] when any of them is no longer there, so
/// nothing is copied in part and the failure says why.
fn to_save(phase: &Phase) -> Result<Vec<ExportFile>, Phase> {
    let Phase::Ready(files) = phase else { return Err(phase.clone()) };
    if files.iter().all(|f| std::path::Path::new(&f.path).is_file()) {
        Ok(files.clone())
    } else {
        Err(Phase::Gone)
    }
}

/// Sends the engine started again by itself (held across a restart), as each last stood. The
/// dialog follows only the jobs it asked for, and the engine names a held send once, at launch,
/// before any dialog is open; the series' dialog takes its send up from here when it opens.
#[derive(Default, Debug)]
pub struct HeldSends(Vec<(JobId, Held)>);

#[derive(Clone, Debug, PartialEq)]
struct Held {
    work: WorkId,
    target: String,
    chapters: Vec<ChapterId>,
    phase: Phase,
}

impl HeldSends {
    /// Follow the engine's word on sends: a send it names with its chapters is kept until it
    /// fails or a dialog takes it up.
    pub fn take(&mut self, event: &ReadingEvent) {
        match event {
            ReadingEvent::ExportWaiting { job, work, target, line, chapters }
                if !chapters.is_empty() =>
            {
                let phase = Phase::Waiting(crate::say::msg(line));
                match self.0.iter_mut().find(|(j, _)| j == job) {
                    Some((_, held)) => held.phase = phase,
                    None => self.0.push((
                        *job,
                        Held {
                            work: work.clone(),
                            target: target.clone(),
                            chapters: chapters.clone(),
                            phase,
                        },
                    )),
                }
            }
            ReadingEvent::ExportProgress { job, done, total, .. } => {
                if let Some((_, held)) = self.0.iter_mut().find(|(j, _)| j == job) {
                    held.phase = Phase::Preparing { done: *done, total: *total };
                }
            }
            ReadingEvent::ExportReady { job, files } => {
                if let Some((_, held)) = self.0.iter_mut().find(|(j, _)| j == job) {
                    held.phase = Phase::Ready(files.clone());
                }
            }
            ReadingEvent::ExportFailed { job, .. } => self.forget(*job),
            _ => {}
        }
    }

    /// A job a dialog follows from now on, or one it stopped.
    fn forget(&mut self, job: JobId) {
        self.0.retain(|(j, _)| *j != job);
    }

    /// The held send for `work`, taken out for its dialog.
    fn take_for(&mut self, work: &WorkId) -> Option<(JobId, Held)> {
        let at = self.0.iter().position(|(_, held)| &held.work == work)?;
        Some(self.0.remove(at))
    }
}

struct Send {
    targets: Vec<ExportTarget>,
    chosen: Option<String>,
    medium: Option<Medium>,
    volumes: Vec<VolumeGroup>,
    /// The next unread (0) or every unread chapter (1).
    scope: u32,
    job: Option<JobId>,
    /// Prepare was asked and its job is not known yet.
    asked: bool,
    sent: Vec<ChapterId>,
    phase: Phase,
}

impl Send {
    /// Follow a held send from where it stands: its job, its e-reader and the chapters it
    /// holds, so saving its files sets the place on them.
    fn take_up(&mut self, job: JobId, held: Held) {
        self.job = Some(job);
        self.asked = false;
        self.chosen = Some(held.target);
        self.sent = held.chapters;
        self.phase = held.phase;
    }

    fn target(&self) -> Option<&ExportTarget> {
        self.targets.iter().find(|t| Some(&t.id) == self.chosen.as_ref())
    }

    /// The format a target gets for this series: the first it takes for pages or text.
    fn format(&self, target: &ExportTarget) -> Option<ExportFormat> {
        let novel = self.medium == Some(Medium::Novel);
        let formats = if novel { &target.novel_formats } else { &target.manga_formats };
        formats.first().or(target.formats.first()).copied()
    }

    /// The chapters the scope picks, in reading order, with their numbers: those not read yet.
    fn chapters(&self) -> Vec<(ChapterId, String)> {
        let unread = self
            .volumes
            .iter()
            .flat_map(|v| &v.chapters)
            .filter(|c| !matches!(c.state, ChapterReadState::Read))
            .map(|c| (c.id.clone(), c.number.clone().unwrap_or_else(|| c.label.clone())));
        if self.scope == 0 { unread.take(NEXT).collect() } else { unread.collect() }
    }
}

fn format_name(format: ExportFormat) -> String {
    match format {
        ExportFormat::FixedEpub => t::send_format_fixed_epub(),
        ExportFormat::Kepub => t::send_format_kepub(),
        ExportFormat::Epub => t::send_format_epub(),
        ExportFormat::Cbz => t::send_format_cbz(),
    }
}

fn heading(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("heading");
    label.set_xalign(0.0);
    label.set_margin_top(6);
    label
}

struct Parts {
    targets: gtk::ListBox,
    checks: RefCell<Vec<(String, gtk::Image)>>,
    what: gtk::ListBox,
    scope: adw::ComboRow,
    ring_slot: gtk::Box,
    state_line: gtk::Box,
    state: gtk::Label,
    under: gtk::Label,
    prepare: gtk::Button,
    again: gtk::Button,
    cancel: gtk::Button,
    save: gtk::Button,
    send: RefCell<Send>,
}

impl Parts {
    /// Whether the ready files are all still there; when any has gone the dialog says so,
    /// with Send again, and false.
    fn still_ready(&self) -> bool {
        let gone = {
            let mut s = self.send.borrow_mut();
            match to_save(&s.phase) {
                Ok(_) => return true,
                Err(Phase::Gone) => {
                    s.phase = Phase::Gone;
                    true
                }
                Err(_) => false,
            }
        };
        if gone {
            self.draw();
        }
        false
    }

    /// Show what the state says.
    fn draw(&self) {
        let s = self.send.borrow();
        let chapters = s.chapters();
        self.scope.set_subtitle(&match (chapters.first(), chapters.last()) {
            _ if s.volumes.is_empty() => String::new(),
            (Some((_, one)), _) if chapters.len() == 1 => t::send_chapters_one(one),
            (Some((_, first)), Some((_, last))) => t::send_chapters_range(first, last),
            _ => t::linux_send_none_unread(),
        });
        let open = matches!(s.phase, Phase::Choosing | Phase::Failed(_) | Phase::Gone);
        self.targets.set_sensitive(open);
        self.what.set_sensitive(open);
        self.prepare.set_visible(s.phase == Phase::Choosing);
        self.prepare.set_sensitive(
            s.target().is_some_and(|t| s.format(t).is_some()) && !chapters.is_empty(),
        );
        self.again.set_visible(matches!(s.phase, Phase::Failed(_) | Phase::Gone));
        self.again.set_label(&if s.phase == Phase::Gone {
            t::send_again()
        } else {
            t::reader_try_again()
        });
        self.cancel.set_visible(matches!(s.phase, Phase::Preparing { .. } | Phase::Waiting(_)));
        self.save.set_visible(matches!(s.phase, Phase::Ready(_)));
        while let Some(child) = self.ring_slot.first_child() {
            self.ring_slot.remove(&child);
        }
        let device = s.target().map(|t| t.name.clone()).unwrap_or_default();
        let (words, line) = match &s.phase {
            Phase::Choosing => (String::new(), String::new()),
            Phase::Preparing { done, total } => {
                let fraction = f64::from(*done) / f64::from((*total).max(1));
                self.ring_slot.append(&widgets::ring(fraction));
                (
                    t::send_preparing(i64::from(*done), i64::from((*total).max(1))),
                    t::send_sized_for(&device),
                )
            }
            Phase::Waiting(line) => (line.clone(), String::new()),
            Phase::Failed(why) => (why.clone(), String::new()),
            Phase::Gone => (t::send_files_gone(), String::new()),
            Phase::Ready(files) => (t::send_ready(files.len() as i64), String::new()),
        };
        self.state.set_label(&words);
        self.state_line.set_visible(!words.is_empty());
        self.under.set_label(&line);
        self.under.set_visible(!line.is_empty());
    }

    /// The e-readers, each with the format and screen it gets and a check on the chosen one.
    fn draw_targets(self: &Rc<Self>) {
        while let Some(row) = self.targets.first_child() {
            self.targets.remove(&row);
        }
        self.checks.borrow_mut().clear();
        {
            let s = self.send.borrow();
            for target in &s.targets {
                let format = s.format(target).map(format_name).unwrap_or_default();
                let row = adw::ActionRow::builder()
                    .title(&target.name)
                    .subtitle(t::send_target_line(
                        &format,
                        &target.width_px.to_string(),
                        &target.height_px.to_string(),
                    ))
                    .activatable(true)
                    .build();
                let check = gtk::Image::from_icon_name("object-select-symbolic");
                row.add_suffix(&check);
                let id = target.id.clone();
                row.connect_activated({
                    let this = Rc::downgrade(self);
                    let id = id.clone();
                    move |_| {
                        let Some(this) = this.upgrade() else { return };
                        this.send.borrow_mut().chosen = Some(id.clone());
                        this.show_checks();
                        this.draw();
                    }
                });
                self.targets.append(&row);
                self.checks.borrow_mut().push((id, check));
            }
        }
        self.show_checks();
        self.draw();
    }

    fn show_checks(&self) {
        let chosen = self.send.borrow().chosen.clone();
        for (id, check) in self.checks.borrow().iter() {
            let on = chosen.as_ref() == Some(id);
            check.set_opacity(if on { 1.0 } else { 0.0 });
            if let Some(row) = check.ancestor(adw::ActionRow::static_type()) {
                row.update_state(&[gtk::accessible::State::Checked(if on {
                    gtk::AccessibleTristate::True
                } else {
                    gtk::AccessibleTristate::False
                })]);
            }
        }
    }
}

pub fn open(ctx: &Rc<Ctx>, work: &WorkId) {
    let dialog = adw::Dialog::new();
    dialog.set_title(&t::send_title());
    // The dialog grows with the targets as they come in; the column's width holds it at 460.
    dialog.set_follows_content_size(true);

    let targets = widgets::boxed_list();
    targets.set_selection_mode(gtk::SelectionMode::None);
    let scope = adw::ComboRow::new();
    scope.set_title(&t::send_chapters());
    scope.set_model(Some(&gtk::StringList::new(&[
        &t::linux_send_next(NEXT as i64),
        &t::linux_send_all_unread(),
    ])));
    let what = widgets::boxed_list();
    what.set_selection_mode(gtk::SelectionMode::None);
    what.append(&scope);
    let note = widgets::haze(&t::send_no_place(), "caption");
    note.set_wrap(true);
    note.set_xalign(0.0);

    // The foot: Prepare files, then how far it is, then the files and Save to a folder.
    let ring_slot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let state = gtk::Label::new(None);
    state.set_wrap(true);
    state.set_xalign(0.0);
    state.set_hexpand(true);
    let state_line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    state_line.append(&ring_slot);
    state_line.append(&state);
    let under = widgets::haze("", "caption");
    under.set_wrap(true);
    under.set_xalign(0.0);
    let pill = |label: &str, amber: bool| {
        let button = gtk::Button::with_label(label);
        button.add_css_class("pill");
        if amber {
            button.add_css_class("suggested-action");
        }
        button
    };
    let prepare = pill(&t::linux_send_prepare(), true);
    let again = pill(&t::reader_try_again(), true);
    let save = pill(&t::linux_send_save(), true);
    let cancel = pill(&t::send_waiting_cancel(), false);
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    buttons.set_halign(gtk::Align::Start);
    for button in [&prepare, &again, &save, &cancel] {
        buttons.append(button);
    }
    let foot = gtk::Box::new(gtk::Orientation::Vertical, 8);
    foot.set_margin_top(10);
    foot.append(&state_line);
    foot.append(&under);
    foot.append(&buttons);

    let column = gtk::Box::new(gtk::Orientation::Vertical, 10);
    column.set_width_request(460 - 2 * 18);
    column.set_margin_top(6);
    column.set_margin_bottom(18);
    column.set_margin_start(18);
    column.set_margin_end(18);
    column.append(&heading(&t::send_for()));
    column.append(&targets);
    column.append(&heading(&t::send_what()));
    column.append(&what);
    column.append(&note);
    column.append(&foot);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(620)
        .child(&column)
        .build();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(&scroll));
    dialog.set_child(Some(&toolbar));

    let parts = Rc::new(Parts {
        targets,
        checks: RefCell::default(),
        what,
        scope: scope.clone(),
        ring_slot,
        state_line,
        state,
        under,
        prepare: prepare.clone(),
        again: again.clone(),
        cancel: cancel.clone(),
        save: save.clone(),
        send: RefCell::new(Send {
            targets: Vec::new(),
            chosen: None,
            medium: None,
            volumes: Vec::new(),
            scope: 0,
            job: None,
            asked: false,
            sent: Vec::new(),
            phase: Phase::Choosing,
        }),
    });
    // A send for this series the engine started again by itself: the dialog shows where it
    // stands and follows it.
    let held = ctx.hub.known.borrow_mut().held_sends.take_for(work);
    if let Some((job, held)) = held {
        parts.send.borrow_mut().take_up(job, held);
    }

    scope.connect_selected_notify({
        let parts = Rc::downgrade(&parts);
        move |scope| {
            let Some(parts) = parts.upgrade() else { return };
            parts.send.borrow_mut().scope = scope.selected();
            parts.draw();
        }
    });

    let start: Rc<dyn Fn()> = Rc::new({
        let (hub, work, parts) = (Rc::downgrade(&ctx.hub), work.clone(), Rc::downgrade(&parts));
        move || {
            let (Some(hub), Some(parts)) = (hub.upgrade(), parts.upgrade()) else { return };
            let (ask, target) = {
                let mut s = parts.send.borrow_mut();
                let Some(target) = s.target().cloned() else { return };
                let Some(format) = s.format(&target) else { return };
                let ids: Vec<ChapterId> = s.chapters().into_iter().map(|(id, _)| id).collect();
                if ids.is_empty() {
                    return;
                }
                s.sent = ids.clone();
                s.job = None;
                s.asked = true;
                s.phase = Phase::Preparing { done: 0, total: ids.len() as u32 };
                let ask = ReadingCommand::Prepare {
                    work: work.clone(),
                    pick: DownloadPick::Chapters { ids },
                    target: target.id.clone(),
                    format,
                };
                (ask, target.id)
            };
            send(&hub, ReadingCommand::SetSendTarget { target });
            send(&hub, ask);
            parts.draw();
        }
    });
    prepare.connect_clicked({
        let start = Rc::clone(&start);
        move |_| start()
    });
    again.connect_clicked(move |_| start());
    cancel.connect_clicked({
        let (hub, parts) = (Rc::downgrade(&ctx.hub), Rc::downgrade(&parts));
        move |_| {
            let Some(parts) = parts.upgrade() else { return };
            let job = {
                let mut s = parts.send.borrow_mut();
                s.asked = false;
                s.phase = Phase::Choosing;
                s.job.take()
            };
            if let (Some(hub), Some(job)) = (hub.upgrade(), job) {
                send(&hub, ReadingCommand::CancelPrepare { job });
            }
            parts.draw();
        }
    });
    save.connect_clicked({
        let (ctx, work, parts) = (Rc::downgrade(ctx), work.clone(), Rc::downgrade(&parts));
        move |_| {
            let (Some(ctx), Some(parts)) = (ctx.upgrade(), parts.upgrade()) else { return };
            if !parts.still_ready() {
                return;
            }
            let chooser = gtk::FileDialog::new();
            let window = ctx.window.clone();
            let work = work.clone();
            let parts = Rc::downgrade(&parts);
            chooser.select_folder(Some(&window), gio::Cancellable::NONE, move |result| {
                let Ok(folder) = result else { return };
                let Some(parts) = parts.upgrade() else { return };
                // The files may have gone while the folder was being picked.
                if !parts.still_ready() {
                    return;
                }
                let (files, chapters, target) = {
                    let s = parts.send.borrow();
                    let Phase::Ready(files) = s.phase.clone() else { return };
                    (files, s.sent.clone(), s.chosen.clone().unwrap_or_default())
                };
                let (ctx, work) = (Rc::clone(&ctx), work.clone());
                glib::spawn_future_local(save_files(
                    ctx, folder, files, work, chapters, target,
                ));
            });
        }
    });

    // Closing while the files are being made stops them.
    dialog.connect_closed({
        let (hub, parts) = (Rc::downgrade(&ctx.hub), Rc::downgrade(&parts));
        move |_| {
            let (Some(hub), Some(parts)) = (hub.upgrade(), parts.upgrade()) else { return };
            let s = parts.send.borrow();
            if matches!(s.phase, Phase::Preparing { .. } | Phase::Waiting(_))
                && let Some(job) = s.job
            {
                send(&hub, ReadingCommand::CancelPrepare { job });
            }
        }
    });

    listen(ctx, &dialog, {
        let (work, parts, hub) = (work.clone(), Rc::clone(&parts), Rc::downgrade(&ctx.hub));
        move |event| {
            let Event::Reading(event) = event else { return };
            // A job this dialog asked for: the first one seen after Prepare, then that one. It is
            // the dialog's to follow, never a held send for the next dialog to take up.
            let ours = |job: &JobId| {
                let mut s = parts.send.borrow_mut();
                if s.asked && s.job.is_none() {
                    s.job = Some(*job);
                }
                let ours = s.job.as_ref() == Some(job);
                if ours && let Some(hub) = hub.upgrade() {
                    hub.known.borrow_mut().held_sends.forget(*job);
                }
                ours
            };
            match event {
                ReadingEvent::ExportTargets(found) => {
                    {
                        let mut s = parts.send.borrow_mut();
                        // A send under way keeps its own e-reader.
                        if s.job.is_none() || s.chosen.is_none() {
                            s.chosen = found
                                .iter()
                                .find(|t| t.chosen)
                                .or(found.first())
                                .map(|t| t.id.clone());
                        }
                        s.targets = found.clone();
                    }
                    parts.draw_targets();
                    return;
                }
                ReadingEvent::Series(view) if view.work == work => {
                    parts.send.borrow_mut().medium = Some(view.medium);
                    parts.draw_targets();
                    return;
                }
                ReadingEvent::Chapters { work: w, volumes, .. } if *w == work => {
                    parts.send.borrow_mut().volumes = volumes.clone();
                }
                ReadingEvent::ExportProgress { job, done, total, chapters_done, chapters }
                    if ours(job) =>
                {
                    let (done, total) = if *chapters > 0 {
                        (*chapters_done, *chapters)
                    } else {
                        (*done, *total)
                    };
                    parts.send.borrow_mut().phase = Phase::Preparing { done, total };
                }
                ReadingEvent::ExportWaiting { job, line, .. } if ours(job) => {
                    parts.send.borrow_mut().phase = Phase::Waiting(crate::say::msg(line));
                }
                ReadingEvent::ExportFailed { job, chapter, failure } if ours(job) => {
                    let cause = super::failure(failure);
                    let words = match chapter {
                        Some(chapter) => t::send_failed_chapter(chapter, &cause),
                        None => t::send_failed(&cause),
                    };
                    let mut s = parts.send.borrow_mut();
                    s.phase = Phase::Failed(words);
                    s.job = None;
                    s.asked = false;
                }
                ReadingEvent::Failed { about: ReadingAbout::Export, failure }
                    if parts.send.borrow().asked =>
                {
                    let mut s = parts.send.borrow_mut();
                    s.phase = Phase::Failed(t::send_failed(&super::failure(failure)));
                    s.asked = false;
                }
                ReadingEvent::ExportReady { job, files } if ours(job) => {
                    let mut s = parts.send.borrow_mut();
                    s.phase = Phase::Ready(files.clone());
                    s.asked = false;
                }
                _ => return,
            }
            parts.draw();
        }
    });
    send(&ctx.hub, ReadingCommand::LoadExportTargets);
    send(&ctx.hub, ReadingCommand::LoadSeries { work: work.clone() });
    send(&ctx.hub, ReadingCommand::LoadChapters { work: work.clone(), edition: None });
    parts.draw();
    dialog.present(Some(&ctx.window));
}

/// Copy the files into `folder` without holding the window, then say so with Show in Files,
/// and tell the engine they went out so the series page can ask where reading stopped.
async fn save_files(
    ctx: Rc<Ctx>,
    folder: gio::File,
    files: Vec<ExportFile>,
    work: WorkId,
    chapters: Vec<ChapterId>,
    target: String,
) {
    let mut saved = 0;
    for file in &files {
        let from = gio::File::for_path(&file.path);
        let to = folder.child(&file.name);
        let (copied, _progress) =
            from.copy_future(&to, gio::FileCopyFlags::OVERWRITE, glib::Priority::DEFAULT);
        match copied.await {
            Ok(()) => saved += 1,
            Err(error) => tracing::warn!(%error, "a prepared file was not saved"),
        }
    }
    if saved < files.len() {
        ctx.alert(&t::linux_send_save_failed());
        return;
    }
    send(&ctx.hub, ReadingCommand::Shared { work, chapters, target });
    let words = t::linux_send_saved(saved as i64);
    match folder.path() {
        Some(path) => {
            ctx.toast_to(
                &words,
                &t::settings_show_in_files(),
                "win.show-folder",
                &path.to_string_lossy(),
            );
        }
        None => ctx.toast(&words),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(dir: &std::path::Path, name: &str) -> ExportFile {
        ExportFile {
            path: dir.join(name).to_string_lossy().into_owned(),
            name: name.to_owned(),
            bytes: 2,
            part: None,
        }
    }

    fn idle() -> Send {
        Send {
            targets: Vec::new(),
            chosen: None,
            medium: None,
            volumes: Vec::new(),
            scope: 0,
            job: None,
            asked: false,
            sent: Vec::new(),
            phase: Phase::Choosing,
        }
    }

    /// A send held across a restart is named once, at launch, with the chapters it holds. The
    /// series' dialog opened later takes it up where it stands, so saving its files sets the
    /// place on those chapters and the e-reader it was for.
    #[test]
    fn a_send_held_across_a_restart_is_taken_up_with_its_chapters() {
        use kaeri_model::{Msg, ReadingMsg};
        let (work, other) = (WorkId::new("a:900302"), WorkId::new("a:900303"));
        let chapters = vec![ChapterId::new("c3"), ChapterId::new("c4")];
        let job = JobId::new(41);
        let mut held = HeldSends::default();
        held.take(&ReadingEvent::ExportWaiting {
            job,
            work: work.clone(),
            target: "kobo".into(),
            line: Msg::Reading(ReadingMsg::ExportWaitsForWifi { n: 2 }),
            chapters: chapters.clone(),
        });
        let dir = tempfile::tempdir().unwrap();
        let files = vec![file(dir.path(), "Made-up Series 3.kepub.epub")];
        held.take(&ReadingEvent::ExportReady { job, files: files.clone() });

        assert!(held.take_for(&other).is_none(), "another series' dialog leaves it");
        let (taken, send) = held.take_for(&work).expect("held for its series");
        let mut s = idle();
        s.take_up(taken, send);
        assert_eq!(s.job, Some(job));
        assert_eq!(s.sent, chapters);
        assert_eq!(s.chosen.as_deref(), Some("kobo"));
        assert_eq!(s.phase, Phase::Ready(files));
        assert!(held.take_for(&work).is_none(), "taken up once");

        // A held send that fails is not kept for a dialog to take up.
        held.take(&ReadingEvent::ExportWaiting {
            job: JobId::new(42),
            work: work.clone(),
            target: "kobo".into(),
            line: Msg::Reading(ReadingMsg::ExportWaitsForWifi { n: 1 }),
            chapters,
        });
        held.take(&ReadingEvent::ExportFailed {
            job: JobId::new(42),
            chapter: None,
            failure: kaeri_model::reading::ReadingFailure::Offline,
        });
        assert!(held.take_for(&work).is_none());
    }

    /// R5A-6. The engine keeps a ready send's files for a day; saving after they went says so,
    /// with Send again, instead of copying some or none of them.
    #[test]
    fn saving_files_that_are_gone_says_they_are_gone() {
        let dir = tempfile::tempdir().unwrap();
        let (one, two) = (
            file(dir.path(), "Made-up Series 1.cbz"),
            file(dir.path(), "Made-up Series 2.cbz"),
        );
        std::fs::write(&one.path, b"pk").unwrap();
        std::fs::write(&two.path, b"pk").unwrap();
        let ready = Phase::Ready(vec![one.clone(), two.clone()]);
        assert_eq!(to_save(&ready), Ok(vec![one.clone(), two.clone()]));

        std::fs::remove_file(&two.path).unwrap();
        assert_eq!(to_save(&ready), Err(Phase::Gone));
        std::fs::remove_file(&one.path).unwrap();
        assert_eq!(to_save(&ready), Err(Phase::Gone));

        // Nothing ready, nothing to save, and the phase stays as it is.
        assert_eq!(to_save(&Phase::Choosing), Err(Phase::Choosing));
    }
}
