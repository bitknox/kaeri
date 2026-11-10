//! The credit pages board's shots on the reading scene: the credit line, a shown credit page and
//! its notices, credit pages in a strip, a chapter in parts, the series' Group row and its
//! picker, and the setting. The fixture engine has no credit pages, parts or groups, so the
//! reader is handed the engine's events for them and the series' events are dressed with groups.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use kaeri_model::{
    Msg, ReadingMsg,
    event::Event,
    reading::{
        BookEditionRow, ChapterRow, GroupChoice, PartGroup, ReadingCommand, ReadingEvent,
    },
};

use crate::{pages::Dest, reading::reader::Reader, window::Shell};

/// Every credit pages shot, in the order a run takes them.
pub const NAMES: [&str; 26] = [
    "read-credit-line",
    "read-credit-chrome",
    "read-credit-shown",
    "read-credit-shown-chrome",
    "read-credit-kept",
    "read-credit-keep-motion",
    "read-credit-failed",
    "read-credit-show-motion",
    "read-credit-end",
    "read-credit-end-shown",
    "read-credit-two",
    "read-credit-two-shown",
    "read-credit-strip",
    "read-credit-strip-shown",
    "read-credit-strip-end",
    "read-credit-strip-end-shown",
    "read-parts",
    "read-parts-scrub",
    "read-parts-missing",
    "read-parts-missing-end",
    "read-series-group",
    "read-series-group-picker",
    "read-series-group-picked",
    "read-series-one-group",
    "read-series-gaps",
    "read-settings-credits",
];

const MANGA: &str = "a:900301";
const STRIP: &str = "a:900303";
const SOURCE: &str = "books.home.example"; // i18n-skip
const GROUPS: [(&str, u32, &str); 3] = [
    ("Lanternlight Scans", 84, "en"), // i18n-skip
    ("Saltmarsh Team", 52, "en"),     // i18n-skip
    ("Bluefinch Group", 19, "es"),    // i18n-skip
];

fn read(shell: &Rc<Shell>, work: &str, chapter: &str) {
    let _ = WidgetExt::activate_action(
        &shell.ctx.window,
        "win.read",
        Some(&(work, chapter).to_variant()),
    );
}

/// Do `what` with the reader `ms` after the stage, once the chapter has opened.
fn later(shell: &Rc<Shell>, ms: u64, what: impl FnOnce(&Rc<Reader>) + 'static) {
    let reader = Rc::clone(&shell.reader);
    gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(ms), move || {
        what(&reader)
    });
}

fn series(shell: &Rc<Shell>, work: &str) {
    let _ = WidgetExt::activate_action(
        &shell.ctx.window,
        "win.open-series",
        Some(&work.to_variant()),
    );
}

/// The first widget named `name` on screen.
fn named(widget: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
    if widget.widget_name() == name && widget.is_mapped() {
        return Some(widget.clone());
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        if let Some(found) = named(&next, name) {
            return Some(found);
        }
        child = next.next_sibling();
    }
    None
}

/// The series' events dressed as a source with `groups` groups would send them: the edition
/// with its groups and the one in use, and with `gaps` the chapters' lines saying which come
/// from another group, in parts, or with a part missing. A pick goes to the engine, which
/// answers with the same events again, dressed with the pick.
fn dress_series(shell: &Rc<Shell>, groups: usize, gaps: bool) {
    let chosen: Rc<RefCell<Option<String>>> = Rc::default();
    let dress: Rc<dyn Fn(&mut Event)> = Rc::new({
        let chosen = Rc::clone(&chosen);
        move |event| match event {
            Event::Reading(ReadingEvent::Series(view)) if view.work.0 == MANGA => {
                view.edition =
                    Some(edition(view.edition.take(), groups, chosen.borrow().clone()));
            }
            Event::Reading(ReadingEvent::Chapters { work, edition: e, volumes, .. })
                if work.0 == MANGA =>
            {
                *e = Some(edition(e.take(), groups, chosen.borrow().clone()));
                if gaps {
                    for chapter in volumes.iter_mut().flat_map(|v| v.chapters.iter_mut()) {
                        gap(chapter);
                    }
                }
            }
            _ => {}
        }
    });
    shell.ctx.hub.dress.replace(Some(dress));
    let answer: crate::hub::ReadAnswer = Rc::new(move |command| {
        if let ReadingCommand::ChooseGroup { group, .. } = command {
            chosen.replace(group.clone());
        }
        None
    });
    shell.ctx.hub.read_answer.replace(Some(answer));
}

/// The edition the series reads from, with `groups` groups, the pick in use when there is one.
fn edition(
    base: Option<BookEditionRow>,
    groups: usize,
    chosen: Option<String>,
) -> BookEditionRow {
    let in_use = chosen.clone().unwrap_or_else(|| GROUPS[0].0.to_owned());
    let choices: Vec<GroupChoice> = GROUPS
        .iter()
        .take(groups)
        .map(|(label, chapters, language)| GroupChoice {
            label: (*label).to_owned(),
            chapters: *chapters,
            in_use: *label == in_use,
            language: Some((*language).to_owned()),
        })
        .collect();
    BookEditionRow {
        source_id: base
            .as_ref()
            .map_or_else(|| "fixture-books".to_owned(), |b| b.source_id.clone()),
        source_name: SOURCE.to_owned(),
        language: Some("en".to_owned()),
        version_label: Some(in_use),
        chapters: 88,
        latest: base.as_ref().and_then(|b| b.latest.clone()),
        preferred: true,
        in_use: true,
        listing: base.and_then(|b| b.listing),
        groups: choices,
        group_chosen: chosen.is_some(),
    }
}

/// A chapter's line as the engine words it for a series in parts and groups.
fn gap(chapter: &mut ChapterRow) {
    let read = |m: ReadingMsg| Msg::Reading(m);
    match chapter.number.as_deref() {
        Some("17") => {
            chapter.other_group = true;
            chapter
                .line
                .push(read(ReadingMsg::ReadingRowFromGroup { group: GROUPS[1].0.to_owned() }));
        }
        Some("16") => {
            chapter.other_group = true;
            chapter.group_language = Some("es".to_owned());
            chapter.line.push(read(ReadingMsg::ReadingRowFromGroupIn {
                group: GROUPS[2].0.to_owned(),
                language: "Spanish".to_owned(), // i18n-skip
            }));
        }
        Some("15") => {
            chapter.parts = 2;
            chapter.part_groups = vec![PartGroup {
                part: 2,
                group: GROUPS[2].0.to_owned(),
                language: Some("es".to_owned()),
            }];
            chapter.line.push(read(ReadingMsg::ReadingRowParts { n: 2 }));
            chapter.line.push(read(ReadingMsg::ReadingRowPartFromGroupIn {
                part: 2,
                group: GROUPS[2].0.to_owned(),
                language: "Spanish".to_owned(), // i18n-skip
            }));
        }
        Some("14") => {
            chapter.parts = 3;
            chapter.missing_parts = vec![2];
            chapter.line.push(read(ReadingMsg::ReadingRowPartsOf { have: 2, n: 3 }));
            chapter.line.push(read(ReadingMsg::ReadingRowPartMissing { count: 1, part: 2 }));
        }
        Some("13") => {
            chapter.parts = 2;
            chapter.line.push(read(ReadingMsg::ReadingRowParts { n: 2 }));
        }
        _ => {}
    }
}

/// Put the window in the state `shot` shows, and say how long to let it settle, when it is one.
pub fn stage(shell: &Rc<Shell>, shot: &str) -> Option<f64> {
    Some(match shot {
        // Page 1 with the credit page before it skipped: the pill, top trailing.
        "read-credit-line" => {
            read(shell, MANGA, "13");
            later(shell, 2500, |reader| {
                reader.shot_go_to_first();
                reader.shot_credits(1, 0);
            });
            3.3
        }
        // The pill back with the chrome, under the header.
        "read-credit-chrome" => {
            read(shell, MANGA, "13");
            later(shell, 2500, |reader| {
                reader.shot_go_to_first();
                reader.shot_credits(1, 0);
                reader.shot_chrome(true);
            });
            3.6
        }
        "read-credit-shown" | "read-credit-shown-chrome" => {
            let chrome = shot == "read-credit-shown-chrome";
            read(shell, MANGA, "13");
            later(shell, 2500, move |reader| {
                reader.shot_go_to_first();
                reader.shot_credits(1, 0);
                reader.shot_show_credits(false);
                if chrome {
                    reader.shot_chrome(true);
                }
            });
            3.8
        }
        // For the motion clip: the pill, then the page shown by a turn's crossfade.
        "read-credit-show-motion" => {
            read(shell, MANGA, "13");
            later(shell, 2500, |reader| {
                reader.shot_go_to_first();
                reader.shot_credits(1, 0);
            });
            later(shell, 3600, |reader| reader.shot_show_credits(false));
            4.6
        }
        // For the motion clip: Not a credit page, Undo, then the keep again, held until its
        // notice times out 8 s later and the keep goes to the engine.
        "read-credit-keep-motion" => {
            read(shell, MANGA, "13");
            later(shell, 2500, |reader| {
                reader.shot_go_to_first();
                reader.shot_credits(1, 0);
            });
            // Shown on a later tick than the turn to page 1, as in read-credit-show-motion: under a
            // recording the turn's own layout lands after it and would clear the shown page.
            later(shell, 3600, |reader| reader.shot_show_credits(false));
            later(shell, 4800, |reader| reader.shot_not_a_credit());
            later(shell, 6800, |reader| reader.shot_undo_keep());
            later(shell, 8300, |reader| reader.shot_not_a_credit());
            17.5
        }
        // The credit page's file can't be read: the pill comes back with a notice.
        "read-credit-failed" => {
            read(shell, MANGA, "13");
            later(shell, 2500, |reader| {
                reader.shot_go_to_first();
                reader.shot_credits_broken();
            });
            later(shell, 3000, |reader| reader.shot_show_credits(false));
            3.8
        }
        // Not a credit page: the page counts at once and the notice offers Undo.
        "read-credit-kept" => {
            read(shell, MANGA, "13");
            later(shell, 2500, |reader| {
                reader.shot_go_to_first();
                reader.shot_credits(1, 0);
            });
            later(shell, 3600, |reader| reader.shot_show_credits(false));
            later(shell, 4400, |reader| reader.shot_not_a_credit());
            5.4
        }
        // A credit page skipped at the end, named on the end page, and shown after the last page.
        "read-credit-end" | "read-credit-end-shown" => {
            let show = shot == "read-credit-end-shown";
            read(shell, MANGA, "13");
            later(shell, 2500, move |reader| {
                reader.shot_credits(0, 1);
                reader.shot_end();
                if show {
                    reader.shot_show_credits(true);
                    reader.shot_chrome(true);
                }
            });
            3.8
        }
        "read-credit-two" | "read-credit-two-shown" => {
            let show = shot == "read-credit-two-shown";
            read(shell, MANGA, "13");
            later(shell, 2500, move |reader| {
                reader.shot_go_to_first();
                reader.shot_credits(2, 0);
                if show {
                    reader.shot_show_credits(false);
                    reader.shot_chrome(true);
                }
            });
            3.8
        }
        // A strip: the row where the credit image was, and the image grown in under its caption.
        "read-credit-strip" | "read-credit-strip-shown" => {
            let show = shot == "read-credit-strip-shown";
            read(shell, STRIP, "1");
            later(shell, 2500, move |reader| {
                reader.shot_credits(1, 0);
                reader.shot_strip_top();
            });
            if show {
                later(shell, 3000, |reader| reader.shot_show_credits(false));
            }
            4.2
        }
        // The marker into the next chapter names the page skipped at the end; Show puts it back
        // above the marker.
        "read-credit-strip-end" | "read-credit-strip-end-shown" => {
            let show = shot == "read-credit-strip-end-shown";
            read(shell, STRIP, "1");
            later(shell, 2500, |reader| {
                reader.shot_credits(0, 1);
                reader.shot_marker(0.62);
            });
            if show {
                later(shell, 3200, |reader| reader.shot_show_credits(true));
            }
            4.4
        }
        // A chapter in two parts: one count, a tick where part 2 starts.
        "read-parts" => {
            read(shell, MANGA, "13");
            later(shell, 2500, |reader| {
                reader.shot_parts(11, false);
                reader.shot_chrome(true);
            });
            3.4
        }
        // A part no group has after the last: its tick at the end, named there.
        "read-parts-missing-end" => {
            read(shell, MANGA, "13");
            later(shell, 2500, |reader| {
                reader.shot_parts_missing_end(8);
                reader.shot_chrome(true);
            });
            later(shell, 2900, |reader| reader.shot_scrub(20));
            3.5
        }
        // The thumb on the tick: the preview names part 2.
        "read-parts-scrub" | "read-parts-missing" => {
            let missing = shot == "read-parts-missing";
            read(shell, MANGA, "13");
            later(shell, 2500, move |reader| {
                reader.shot_parts(if missing { 8 } else { 11 }, missing);
                reader.shot_chrome(true);
            });
            // Once the group is on screen, as a drag can only start then.
            later(shell, 2900, move |reader| reader.shot_scrub(if missing { 14 } else { 11 }));
            3.5
        }
        "read-series-group" => {
            dress_series(shell, 3, false);
            series(shell, MANGA);
            3.5
        }
        "read-series-group-picker" => {
            dress_series(shell, 3, false);
            series(shell, MANGA);
            open_picker(shell, 2800);
            3.8
        }
        // After picking Saltmarsh Team: the row says it is the person's pick.
        "read-series-group-picked" => {
            dress_series(shell, 3, true);
            series(shell, MANGA);
            open_picker(shell, 2500);
            let window = shell.ctx.window.clone();
            gtk::glib::timeout_add_local_once(
                std::time::Duration::from_millis(2900),
                move || {
                    pick(&window, 3);
                },
            );
            4.6
        }
        "read-series-one-group" => {
            dress_series(shell, 1, false);
            series(shell, MANGA);
            3.5
        }
        "read-series-gaps" => {
            dress_series(shell, 3, true);
            series(shell, MANGA);
            3.5
        }
        "read-settings-credits" => {
            let _ = WidgetExt::activate_action(
                &shell.ctx.window,
                "win.settings-pane",
                Some(&"reading".to_variant()),
            );
            2.5
        }
        _ => return None,
    })
}

/// Leave what `shot` opened: the reader, the dressed events and the series.
pub fn unstage(shell: &Rc<Shell>, shot: &str) {
    if !NAMES.contains(&shot) {
        return;
    }
    if let Some(dialog) = shell.ctx.window.visible_dialog() {
        dialog.force_close();
    }
    shell.reader.shot_chrome(false);
    shell.reader.close();
    shell.ctx.hub.dress.replace(None);
    shell.ctx.hub.read_answer.replace(None);
    shell.go(Dest::Read);
}

/// The Group picker when it is open, placed over the window under its button or above it.
pub fn shown_popover(window: &adw::ApplicationWindow) -> Option<(gtk::Widget, f32, f32)> {
    fn find(widget: &gtk::Widget) -> Option<gtk::Popover> {
        if let Some(popover) = widget.downcast_ref::<gtk::Popover>()
            && popover.has_css_class("group-picker")
            && popover.is_visible()
            && popover.parent().is_some_and(|p| p.is_mapped())
        {
            return Some(popover.clone());
        }
        let mut child = widget.first_child();
        while let Some(next) = child {
            if let Some(found) = find(&next) {
                return Some(found);
            }
            child = next.next_sibling();
        }
        None
    }
    let popover = find(window.upcast_ref())?;
    let bounds = popover.parent()?.compute_bounds(window)?;
    let width = popover.width() as f32;
    let middle = bounds.x() + bounds.width() / 2.0;
    let x = (middle - width / 2.0).clamp(0.0, (window.width() as f32 - width).max(0.0));
    // Below the button, or above it when the window has no room below, as GTK flips it.
    let below = bounds.y() + bounds.height();
    let y = if below + popover.height() as f32 > window.height() as f32 {
        bounds.y() - popover.height() as f32
    } else {
        below
    };
    Some((popover.upcast(), x, y.max(0.0)))
}

/// Click the Group row's Change `ms` after the stage.
fn open_picker(shell: &Rc<Shell>, ms: u64) {
    let window = shell.ctx.window.clone();
    gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(ms), move || {
        if let Some(button) = named(window.upcast_ref(), "series-group-change")
            .and_then(|w| w.downcast::<gtk::MenuButton>().ok())
        {
            // The page scrolled so the row sits high enough for the whole picker under it.
            if let Some(scroll) = button.ancestor(gtk::ScrolledWindow::static_type())
                && let Ok(scroll) = scroll.downcast::<gtk::ScrolledWindow>()
            {
                scroll.vadjustment().set_value(300.0);
            }
            let button = button.clone();
            gtk::glib::timeout_add_local_once(
                std::time::Duration::from_millis(150),
                move || {
                    button.popup();
                },
            );
        }
    });
}

/// Pick the picker's row `index`, as a click would.
fn pick(window: &adw::ApplicationWindow, index: i32) {
    if let Some(list) = find_list(window.upcast_ref())
        && let Some(row) = list.row_at_index(index)
    {
        row.activate();
    }
}

fn find_list(widget: &gtk::Widget) -> Option<gtk::ListBox> {
    if let Some(list) = widget.downcast_ref::<gtk::ListBox>()
        && list.has_css_class("group-picker-list")
    {
        return Some(list.clone());
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        if let Some(found) = find_list(&next) {
            return Some(found);
        }
        child = next.next_sibling();
    }
    None
}
