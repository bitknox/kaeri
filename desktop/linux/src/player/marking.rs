//! Marking skip times (Alt+K), the prompts about them and the Skip times list. The engine holds
//! the steps and the rules; this draws the bar it describes and sends the presses back.

use std::time::Duration;

use adw::prelude::*;
use gtk::{gdk, glib};
use kaeri_i18n::t;
use kaeri_model::{
    command::PlayerCommand,
    playback::{
        SkipEdge, SkipKind, SkipMarkCheck, SkipMarkNote, SkipMarkStep, SkipMarkView,
        SkipNotice, SkipOrigin, SkipPrompt,
    },
};

use super::PlayerView;
use crate::{say, widgets};

/// Wrong times is up for 8 s, Was that the opening for as long as the corner's other notes.
const WRONG_FOR: Duration = Duration::from_secs(8);
const WAS_OPENING_FOR: Duration =
    Duration::from_secs(kaeri_model::playback::SKIP_PROMPT_HOLD_SECS as u64);
/// How long the found times' card holds its answer.
const FOUND_ANSWER_FOR: Duration =
    Duration::from_secs(kaeri_model::playback::FOUND_ANSWER_SECS as u64);

/// A prompt's button: its words, whether it is the one to press, and what it does.
type PromptButton = (String, bool, Box<dyn Fn(&PlayerView)>);

const KINDS: [SkipKind; 3] = [SkipKind::Opening, SkipKind::Ending, SkipKind::Recap];

pub fn kind_name(kind: SkipKind) -> String {
    match kind {
        SkipKind::Opening => t::player_mark_kind_opening(),
        SkipKind::Ending => t::player_mark_kind_ending(),
        SkipKind::Recap => t::player_mark_kind_recap(),
    }
}

fn kind_inline(kind: SkipKind) -> String {
    match kind {
        SkipKind::Opening => t::player_mark_kind_opening_inline(),
        SkipKind::Ending => t::player_mark_kind_ending_inline(),
        SkipKind::Recap => t::player_mark_kind_recap_inline(),
    }
}

fn kind_plural(kind: SkipKind) -> String {
    match kind {
        SkipKind::Opening => t::player_mark_kind_opening_plural(),
        SkipKind::Ending => t::player_mark_kind_ending_plural(),
        SkipKind::Recap => t::player_mark_kind_recap_plural(),
    }
}

/// "1 min 30 s": the review's length.
fn length(seconds: f64) -> String {
    let s = seconds.max(0.0).round() as i64;
    let (minutes, rest) = (s / 60, s % 60);
    if minutes == 0 {
        t::duration_seconds(rest)
    } else if rest > 0 {
        t::duration_minutes_seconds(minutes, &format!("{rest:02}"))
    } else {
        t::duration_minutes(minutes)
    }
}

fn most(seconds: f64) -> String {
    t::time_minutes(((seconds / 60.0).round() as i64).max(1))
}

/// The notice an action on the skip times says once, on the video.
pub fn notice_text(notice: &SkipNotice) -> String {
    match notice {
        SkipNotice::Saved { kind } => t::player_mark_saved(&kind_inline(*kind)),
        SkipNotice::SavedHere { kind, most: m } => {
            t::player_mark_too_long(&kind_plural(*kind), &most(*m))
        }
        SkipNotice::Confirmed => t::player_mark_confirmed(),
        SkipNotice::Reported => t::player_skip_reported(),
        SkipNotice::HiddenHere => t::player_skip_hidden_here(),
    }
}

/// A button for the bar and the prompts: 34 px tall, a pill, amber when it is the next thing to
/// do.
fn pill(text: &str, suggested: bool) -> gtk::Button {
    let button = gtk::Button::with_label(text);
    button.add_css_class("pill");
    button.add_css_class("mark-button");
    if suggested {
        button.add_css_class("suggested-action");
    }
    button
}

fn numeric(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("numeric");
    label.set_xalign(0.0);
    label
}

impl PlayerView {
    /// Alt+K and the menu entry: open the bar for the kind the playhead suggests, or close it.
    pub(super) fn toggle_marking(&self) {
        if self.marking.borrow().is_some() {
            self.send(PlayerCommand::CancelSkipMark);
        } else {
            self.begin_marking(None);
        }
    }

    fn begin_marking(&self, kind: Option<SkipKind>) {
        if self.ctx.hub.known.borrow().party.is_some() {
            self.ctx.toast(&t::player_mark_unavailable());
            return;
        }
        self.send(PlayerCommand::BeginSkipMark { kind });
    }

    /// Keys while the bar is up. `None` when it is not, or the key is not one of its own.
    pub(super) fn marking_key(&self, key: gdk::Key, shift: bool) -> Option<bool> {
        use gdk::Key;
        let step = self.marking.borrow().as_ref()?.step;
        match key {
            Key::Left | Key::Right if step != SkipMarkStep::Review => {
                let seconds = if shift { 5 } else { 1 };
                let by_ms = if key == Key::Left { -seconds } else { seconds } * 1000;
                let edge =
                    if step == SkipMarkStep::Start { SkipEdge::Start } else { SkipEdge::End };
                self.send(PlayerCommand::NudgeSkipEdge { edge, by_ms });
                Some(true)
            }
            Key::Return | Key::KP_Enter => {
                if step == SkipMarkStep::Review {
                    if self.marking.borrow().as_ref().is_some_and(|v| v.can_save) {
                        self.send(PlayerCommand::SaveSkipMark);
                    }
                } else {
                    self.send(PlayerCommand::MarkSkipEdge);
                }
                Some(true)
            }
            Key::Escape => {
                self.send(PlayerCommand::CancelSkipMark);
                Some(true)
            }
            _ => None,
        }
    }

    /// What the engine says about the marking bar.
    pub(super) fn skip_mark(&self, view: Option<SkipMarkView>) {
        let closed = view.is_none();
        self.marking.replace(view);
        self.draw_marking();
        self.update_provisional();
        if closed {
            self.ui.page.grab_focus();
        } else {
            self.wake();
        }
    }

    /// The segment being marked, drawn on the seek line at half water: from the start to the
    /// playhead, then to the end.
    pub(super) fn update_provisional(&self) {
        let stretch = match self.marking.borrow().as_ref() {
            Some(SkipMarkView { start: Some(start), step: SkipMarkStep::End, .. }) => {
                let at = self.state.borrow().position.unwrap_or(*start);
                Some((*start, at.max(*start)))
            }
            Some(SkipMarkView {
                start: Some(start),
                end: Some(end),
                step: SkipMarkStep::Review,
                ..
            }) => Some((start.min(*end), start.max(*end))),
            _ => None,
        };
        self.ui.marked.set_provisional(stretch);
    }

    fn click(&self, button: &gtk::Button, act: impl Fn(&PlayerView) + 'static) {
        let weak = self.self_weak();
        button.connect_clicked(move |_| {
            if let Some(view) = weak.upgrade() {
                act(&view);
            }
        });
    }

    fn draw_marking(&self) {
        let bar = &self.ui.mark_bar;
        while let Some(child) = bar.first_child() {
            bar.remove(&child);
        }
        let view = self.marking.borrow().clone();
        let Some(view) = view else {
            bar.set_visible(false);
            return;
        };
        bar.update_property(&[gtk::accessible::Property::Label(&t::player_mark_skip_times())]);

        let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        if view.step == SkipMarkStep::Review {
            let title = gtk::Label::new(Some(&t::player_mark_review_title(
                &kind_name(view.kind),
                &length(view.length),
            )));
            title.add_css_class("heading");
            title.set_xalign(0.0);
            title.set_hexpand(true);
            head.append(&title);
        } else {
            let choice = self.kind_choice(&view);
            choice.set_hexpand(true);
            head.append(&choice);
        }
        let cancel = gtk::Button::from_icon_name("window-close-symbolic");
        cancel.add_css_class("circular");
        cancel.add_css_class("flat");
        cancel.set_tooltip_text(Some(&t::action_cancel()));
        self.click(&cancel, |v| v.send(PlayerCommand::CancelSkipMark));
        head.append(&cancel);
        bar.append(&head);

        if view.step == SkipMarkStep::Review {
            self.review(&view, bar);
        } else {
            self.steps(&view, bar);
        }
        bar.set_visible(true);
        bar.set_opacity(1.0);
    }

    /// The segmented control: the kind being marked.
    fn kind_choice(&self, view: &SkipMarkView) -> gtk::Box {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        row.add_css_class("linked");
        row.set_halign(gtk::Align::Start);
        let mut first: Option<gtk::ToggleButton> = None;
        for kind in KINDS {
            let choice = gtk::ToggleButton::with_label(&kind_name(kind));
            choice.set_active(kind == view.kind);
            match &first {
                Some(first) => choice.set_group(Some(first)),
                None => first = Some(choice.clone()),
            }
            let current = view.kind;
            let weak = self.self_weak();
            choice.connect_clicked(move |button| {
                let Some(view) = weak.upgrade() else { return };
                if kind == current {
                    button.set_active(true);
                } else {
                    view.send(PlayerCommand::SetSkipMarkKind { kind });
                }
            });
            row.append(&choice);
        }
        row
    }

    /// The first two steps: what to do, and the one button that does it.
    fn steps(&self, view: &SkipMarkView, bar: &gtk::Box) {
        let start = view.step == SkipMarkStep::Start;
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let line = gtk::Box::new(gtk::Orientation::Vertical, 2);
        line.set_hexpand(true);
        line.set_valign(gtk::Align::Center);
        let kind = kind_inline(view.kind);
        let words = gtk::Label::new(Some(&if start {
            t::player_mark_step_start(&kind)
        } else {
            t::player_mark_step_end(&kind)
        }));
        words.set_wrap(true);
        words.set_xalign(0.0);
        words.set_max_width_chars(36);
        line.append(&words);
        if let Some(at) = view.start {
            let starts = widgets::haze(
                &format!("{} {}", t::player_mark_starts(), say::clock(at)),
                "caption",
            );
            starts.add_css_class("numeric");
            line.append(&starts);
        }
        row.append(&line);
        let mark =
            pill(&if start { t::player_mark_start() } else { t::player_mark_end() }, true);
        mark.set_valign(gtk::Align::Center);
        self.click(&mark, |v| v.send(PlayerCommand::MarkSkipEdge));
        row.append(&mark);
        bar.append(&row);
    }

    /// The review: the edges with their nudges, what saving does, Watch the edges and Save.
    fn review(&self, view: &SkipMarkView, bar: &gtk::Box) {
        bar.append(&self.edge(
            &t::player_mark_starts(),
            &t::player_mark_edge_start(),
            SkipEdge::Start,
            view.start.unwrap_or(0.0),
        ));
        bar.append(&self.edge(
            &t::player_mark_ends(),
            &t::player_mark_edge_end(),
            SkipEdge::End,
            view.end.unwrap_or(0.0),
        ));
        let note = |text: String| {
            let line = widgets::haze(&text, "caption");
            line.set_max_width_chars(48);
            bar.append(&line);
        };
        match view.check {
            SkipMarkCheck::TooShort => note(t::player_mark_too_short()),
            SkipMarkCheck::TooLong { most: m } => {
                note(t::player_mark_too_long(&kind_plural(view.kind), &most(m)));
            }
            SkipMarkCheck::Fine => {}
        }
        for n in &view.notes {
            match n {
                SkipMarkNote::Shares => note(t::player_mark_shares()),
                SkipMarkNote::Replaces { start, end } => {
                    note(t::player_mark_replaces(&say::clock(*start), &say::clock(*end)));
                }
                SkipMarkNote::NoEntry => note(t::player_mark_no_mal()),
                SkipMarkNote::Found => note(t::player_mark_found()),
            }
        }
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_margin_top(4);
        let watch = pill(&t::player_mark_watch_edges(), false);
        watch.set_sensitive(!view.previewing);
        watch.set_hexpand(true);
        watch.set_halign(gtk::Align::Start);
        self.click(&watch, |v| v.send(PlayerCommand::PreviewSkipEdges));
        let save = pill(&t::player_mark_save(), true);
        save.set_sensitive(view.can_save);
        self.click(&save, |v| v.send(PlayerCommand::SaveSkipMark));
        actions.append(&watch);
        actions.append(&save);
        bar.append(&actions);
    }

    fn edge(&self, label: &str, name: &str, edge: SkipEdge, at: f64) -> gtk::Box {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let title = widgets::haze(label, "");
        title.set_width_chars(8);
        title.set_valign(gtk::Align::Center);
        row.append(&title);
        let time = numeric(&say::clock(at));
        time.set_hexpand(true);
        time.set_valign(gtk::Align::Center);
        row.append(&time);
        let nudge = |text: String, spoken: String, by_ms: i64| {
            let button = pill(&text, false);
            button.set_width_request(64);
            button.update_property(&[gtk::accessible::Property::Label(&spoken)]);
            self.click(&button, move |v| v.send(PlayerCommand::NudgeSkipEdge { edge, by_ms }));
            row.append(&button);
        };
        nudge(t::player_mark_back_1(), t::player_mark_back_1_label(name), -1000);
        nudge(t::player_mark_forward_1(), t::player_mark_forward_1_label(name), 1000);
        row
    }

    // The prompts: Was that the opening, Wrong times, and the one-time sharing question.

    pub(super) fn show_prompt(&self, prompt: Option<SkipPrompt>) {
        if let Some(source) = self.prompt_timer.take() {
            source.remove();
        }
        if matches!(prompt, Some(SkipPrompt::Share)) {
            self.skip_prompt.set(None);
            self.ui.skip_prompt.set(false);
            self.ask_to_share();
            return;
        }
        self.skip_prompt.set(prompt);
        let Some(prompt) = prompt else {
            self.ui.skip_prompt.set(false);
            return;
        };
        self.build_prompt(prompt);
        self.ui.skip_prompt.set(true);
        if self.prompt_held.get() {
            return;
        }
        let after = match prompt {
            SkipPrompt::WrongTimes { .. } => WRONG_FOR,
            SkipPrompt::FoundSaved { .. } | SkipPrompt::FoundNever => FOUND_ANSWER_FOR,
            _ => WAS_OPENING_FOR,
        };
        let weak = self.self_weak();
        let source = glib::timeout_add_local_once(after, move || {
            let Some(view) = weak.upgrade() else { return };
            view.prompt_timer.take();
            if view.skip_prompt.take().is_some() {
                view.ui.skip_prompt.set(false);
                view.send(PlayerCommand::DismissSkipPrompt);
            }
        });
        self.prompt_timer.replace(Some(source));
    }

    fn build_prompt(&self, prompt: SkipPrompt) {
        let card = &self.ui.skip_prompt.widget;
        while let Some(child) = card.first_child() {
            card.remove(&child);
        }
        let (line, buttons): (String, Vec<PromptButton>) = match prompt {
            SkipPrompt::WasOpening { .. } => (
                t::player_was_opening(),
                vec![
                    (
                        t::player_dont_ask_again(),
                        false,
                        Box::new(|v| v.send(PlayerCommand::NeverAskWasOpening)),
                    ),
                    (
                        t::player_mark_it(),
                        true,
                        Box::new(|v| v.begin_marking(Some(SkipKind::Opening))),
                    ),
                ],
            ),
            SkipPrompt::WrongTimes { kind } => (
                t::player_skip_wrong(&kind_inline(kind)),
                vec![
                    (
                        t::player_skip_report(),
                        false,
                        Box::new(move |v| v.send(PlayerCommand::ReportSkip { kind })),
                    ),
                    (
                        t::player_skip_mark(),
                        true,
                        Box::new(move |v| v.begin_marking(Some(kind))),
                    ),
                ],
            ),
            SkipPrompt::FoundEdges { kind, episode } => (
                format!(
                    "{}\n{}",
                    match kind {
                        SkipKind::Ending => t::player_found_ask_ending(),
                        _ => t::player_found_ask_opening(),
                    },
                    t::player_found_ask_share(episode.into())
                ),
                vec![
                    (
                        t::player_dont_ask_again(),
                        false,
                        Box::new(|v| v.send(PlayerCommand::NeverAskWasOpening)),
                    ),
                    (
                        t::player_found_adjust(),
                        false,
                        Box::new(move |v| v.send(PlayerCommand::AdjustFoundSkip { kind })),
                    ),
                    (
                        t::player_found_yes(),
                        true,
                        Box::new(move |v| v.send(PlayerCommand::ConfirmFoundSkip { kind })),
                    ),
                ],
            ),
            SkipPrompt::FoundSaved { episode, .. } => {
                (t::player_found_saved(episode.into()), Vec::new())
            }
            SkipPrompt::FoundNever => (
                format!("{}\n{}", t::player_found_never(), t::player_found_never_how()),
                Vec::new(),
            ),
            SkipPrompt::Share => return,
        };
        // The found times' card is answered as it stands: the engine reads which card it was.
        let answered = matches!(prompt, SkipPrompt::FoundEdges { .. });
        let label = gtk::Label::new(Some(&line));
        label.set_valign(gtk::Align::Center);
        label.set_wrap(true);
        label.set_max_width_chars(28);
        label.set_xalign(0.0);
        card.append(&label);
        for (text, suggested, act) in buttons {
            let button = pill(&text, suggested);
            let weak = self.self_weak();
            button.connect_clicked(move |_| {
                let Some(view) = weak.upgrade() else { return };
                view.skip_prompt.set(None);
                view.ui.skip_prompt.set(false);
                if let Some(source) = view.prompt_timer.take() {
                    source.remove();
                }
                if !answered {
                    view.send(PlayerCommand::DismissSkipPrompt);
                }
                act(&view);
            });
            card.append(&button);
        }
    }

    /// The first Save asks once: share the times with AniSkip, or keep them on the viewer's
    /// devices.
    fn ask_to_share(&self) {
        if self.sharing.replace(true) {
            return;
        }
        let dialog = adw::AlertDialog::new(
            Some(&t::player_share_title()),
            Some(&format!("{}\n\n{}", t::player_share_body(), t::player_share_sent())),
        );
        dialog.add_response("keep", &t::player_share_no());
        dialog.add_response("share", &t::player_share_yes());
        dialog.set_response_appearance("share", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("share"));
        dialog.set_close_response("keep");
        let weak = self.self_weak();
        dialog.connect_response(None, move |_, response| {
            let Some(view) = weak.upgrade() else { return };
            view.sharing.set(false);
            view.send(PlayerCommand::AnswerShareSkipTimes { share: response == "share" });
        });
        dialog.present(Some(&self.ctx.window));
        self.skip_dialog.replace(Some(dialog.upcast()));
    }

    // The Skip times list: where each time came from, and what can be done to it.

    pub(super) fn show_skip_times(&self) {
        let list = gtk::ListBox::new();
        list.add_css_class("boxed-list");
        list.set_selection_mode(gtk::SelectionMode::None);
        let dialog = adw::Dialog::builder()
            .title(t::player_skip_times())
            .content_width(460)
            .follows_content_size(true)
            .build();
        for kind in KINDS {
            let found = self.timeline_marks.borrow().iter().find(|m| m.kind == kind).copied();
            let origin = match found {
                None => t::player_skip_none(),
                Some(m) => match m.origin {
                    Some(SkipOrigin::Yours) => t::player_skip_yours(),
                    Some(SkipOrigin::AniSkip) => t::player_skip_from_aniskip(),
                    Some(SkipOrigin::Found) => t::player_skip_found(),
                    _ => t::player_skip_from_file(),
                },
            };
            let subtitle = match found {
                Some(m) => {
                    format!("{}\u{2013}{} · {origin}", say::clock(m.start), say::clock(m.end)) // i18n-skip
                }
                None => origin,
            };
            let row =
                adw::ActionRow::builder().title(kind_name(kind)).subtitle(subtitle).build();
            row.add_css_class("numeric");
            let suffix = |text: String, act: Box<dyn Fn(&PlayerView)>| {
                let button = gtk::Button::with_label(&text);
                button.set_valign(gtk::Align::Center);
                let (weak, dialog) = (self.self_weak(), dialog.clone());
                button.connect_clicked(move |_| {
                    dialog.close();
                    if let Some(view) = weak.upgrade() {
                        act(&view);
                    }
                });
                row.add_suffix(&button);
            };
            if let Some(m) = found {
                match m.origin {
                    Some(SkipOrigin::AniSkip) => suffix(
                        t::player_skip_report(),
                        Box::new(move |v| v.send(PlayerCommand::ReportSkip { kind })),
                    ),
                    Some(SkipOrigin::Yours) => suffix(
                        t::player_skip_clear(),
                        Box::new(move |v| v.send(PlayerCommand::ClearMySkip { kind })),
                    ),
                    Some(SkipOrigin::Found) => suffix(
                        t::player_found_adjust(),
                        Box::new(move |v| v.send(PlayerCommand::AdjustFoundSkip { kind })),
                    ),
                    _ => {}
                }
            }
            suffix(t::player_skip_mark(), Box::new(move |v| v.begin_marking(Some(kind))));
            list.append(&row);
        }
        let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
        column.set_margin_top(12);
        column.set_margin_bottom(18);
        column.set_margin_start(18);
        column.set_margin_end(18);
        column.append(&list);
        let view = adw::ToolbarView::new();
        view.add_top_bar(&adw::HeaderBar::new());
        view.set_content(Some(&column));
        dialog.set_child(Some(&view));
        dialog.present(Some(&self.ctx.window));
        self.skip_dialog.replace(Some(dialog));
    }

    /// For a shot: everything the last one left up comes down.
    #[cfg(feature = "fixtures")]
    pub fn shot_reset(&self) {
        if let Some(dialog) = self.skip_dialog.take() {
            dialog.force_close();
        }
        self.sharing.set(false);
        self.prompt_held.set(false);
        self.show_prompt(None);
        self.skip_mark(None);
    }

    /// For a shot: the bar of `fixture`, as the engine's fixtures describe it.
    #[cfg(feature = "fixtures")]
    pub fn shot_marking(&self, fixture: kaeri_model::playback::SkipMarkFixture) {
        // The shot's clip is short: the times sit inside it.
        let mut view = fixture.view();
        view.start = view.start.map(|_| 10.0);
        view.end = view.end.map(|_| 34.0);
        if view.end.is_some() {
            view.length = 24.0;
        }
        self.skip_mark(Some(view));
    }

    /// For a shot: a prompt, kept up whatever the engine says.
    #[cfg(feature = "fixtures")]
    pub fn shot_prompt(&self, prompt: SkipPrompt) {
        self.prompt_held.set(true);
        self.show_prompt(Some(prompt));
        self.wake();
    }

    /// For a shot: the Skip times list over an opening of the viewer's and one from AniSkip.
    #[cfg(feature = "fixtures")]
    pub fn shot_skip_times(&self) {
        use kaeri_model::playback::TimelineMark;
        self.timeline_marks.replace(vec![
            TimelineMark {
                kind: SkipKind::Opening,
                start: 91.0,
                end: 181.0,
                origin: Some(SkipOrigin::AniSkip),
            },
            TimelineMark {
                kind: SkipKind::Ending,
                start: 1290.0,
                end: 1380.0,
                origin: Some(SkipOrigin::Yours),
            },
        ]);
        self.show_skip_times();
    }
}
