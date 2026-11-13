//! The reading shots: each Read place, a series' page, the reader in its three forms and Send
//! to e-reader, on the reading fixture scene (`--scene reading`). The light ones are the reader's
//! own light looks, as the app's chrome is dark only.

use std::rc::Rc;

use adw::prelude::*;
use kaeri_core::books::WorkId;
use kaeri_model::{
    input::{Direction, ReaderPad},
    reader::ReaderSettingKey,
};

use crate::{pages::Dest, reading::reader::Reader, window::Shell};

/// Every reading shot, in the order a run takes them.
pub const NAMES: [&str; 49] = [
    "read",
    "read-updates",
    "read-library",
    "read-library-filters",
    "read-search",
    "read-search-query",
    "read-settings",
    "read-medium-off",
    "read-search-filters",
    "read-downloads",
    "read-sources",
    "read-series",
    "read-series-source",
    "read-paged",
    "read-paged-light",
    "read-paged-crop",
    "read-paged-single",
    "read-paged-width",
    "read-paged-settings",
    // Tab after the chrome has gone brings it, with the focus on its first control.
    "read-paged-tab",
    "read-end",
    "read-end-none",
    "read-end-plate",
    "read-end-skip",
    "read-plate",
    "read-open-failed",
    // Try again on a chapter that couldn't open opens it again.
    "read-open-again",
    "read-mode",
    "read-turn-a",
    "read-turn-b",
    "read-strip",
    "read-strip-plate",
    "read-strip-plate-slow",
    "read-strip-slide-a",
    "read-strip-slide-b",
    "read-strip-end",
    "read-strip-mode",
    "read-strip-light",
    // Last of the strip's: running on reads chapter 2 to its end, and the marker shot opens it
    // at its foot after that.
    "read-strip-flowed",
    "read-strip-marker",
    "read-chapters",
    "read-chapters-list",
    // Where a novel's notice sits, with the chrome up and gone.
    "read-novel-notice",
    "read-novel-notice-hidden",
    "read-novel",
    "read-novel-light",
    "read-novel-paper",
    // A novel read to its end page counts as read, and Continue opens the next chapter.
    "read-novel-end",
    "read-send",
];

const MANGA: &str = "a:900301";
const NOVEL: &str = "a:900302";
const STRIP: &str = "a:900303";

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

/// Type `query` into the search field on screen, once the page has come in.
fn type_query(shell: &Rc<Shell>, query: &'static str) {
    fn entry(widget: &gtk::Widget) -> Option<gtk::SearchEntry> {
        if widget.is_mapped()
            && let Some(entry) = widget.downcast_ref::<gtk::SearchEntry>()
        {
            return Some(entry.clone());
        }
        let mut child = widget.first_child();
        while let Some(next) = child {
            if let Some(found) = entry(&next) {
                return Some(found);
            }
            child = next.next_sibling();
        }
        None
    }
    let window = shell.ctx.window.clone();
    gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(1200), move || {
        if let Some(entry) = entry(window.upcast_ref()) {
            entry.set_text(query);
        }
    });
}

fn settings_reading(shell: &Rc<Shell>) {
    let _ = WidgetExt::activate_action(
        &shell.ctx.window,
        "win.settings-pane",
        Some(&"reading".to_variant()),
    );
}

/// Put the window in the state `shot` shows, and say how long to let it settle, when it is one.
pub fn stage(shell: &Rc<Shell>, shot: &str) -> Option<f64> {
    Some(match shot {
        "read" => {
            shell.go(Dest::Read);
            3.0
        }
        "read-updates" => {
            shell.go(Dest::Updates);
            3.0
        }
        "read-library" => {
            shell.go(Dest::ReadLibrary);
            3.0
        }
        "read-library-filters" => {
            shell.go(Dest::ReadLibrary);
            crate::reading::library::shot_filters(shell);
            3.0
        }
        "read-search" => {
            shell.go(Dest::ReadSearch);
            3.0
        }
        "read-search-query" => {
            shell.go(Dest::ReadSearch);
            type_query(shell, "Lantern");
            4.5
        }
        "read-settings" => {
            settings_reading(shell);
            3.0
        }
        "read-medium-off" => {
            settings_reading(shell);
            let ctx = Rc::clone(&shell.ctx);
            gtk::glib::timeout_add_local_once(
                std::time::Duration::from_millis(1500),
                move || {
                    crate::reading::medium::shot_turn_off(&ctx);
                },
            );
            3.5
        }
        "read-search-filters" => {
            shell.go(Dest::ReadSearch);
            crate::reading::search::shot_filters(shell);
            3.5
        }
        "read-downloads" => {
            shell.go(Dest::ReadDownloads);
            3.0
        }
        "read-sources" => {
            shell.go(Dest::ReadSources);
            3.0
        }
        "read-series" => {
            series(shell, MANGA);
            3.5
        }
        "read-series-source" => {
            series(shell, MANGA);
            crate::reading::series::shot_source_picker(shell);
            3.5
        }
        "read-paged" => {
            read(shell, MANGA, "13");
            4.0
        }
        "read-paged-light" => {
            shell.reader.shot_setting(ReaderSettingKey::Background, "white");
            read(shell, MANGA, "13");
            4.0
        }
        "read-paged-crop" => {
            shell.reader.shot_setting(ReaderSettingKey::CropBorders, "on");
            read(shell, MANGA, "13");
            4.0
        }
        "read-paged-single" => {
            shell.reader.shot_setting(ReaderSettingKey::PageLayout, "single");
            read(shell, MANGA, "13");
            4.0
        }
        "read-paged-width" => {
            shell.reader.shot_setting(ReaderSettingKey::PageLayout, "single");
            shell.reader.shot_setting(ReaderSettingKey::Fit, "width");
            read(shell, MANGA, "13");
            4.0
        }
        "read-paged-tab" => {
            read(shell, MANGA, "13");
            later(shell, 4500, |reader| reader.shot_tab());
            6.0
        }
        "read-paged-settings" => {
            read(shell, MANGA, "13");
            let reader = Rc::clone(&shell.reader);
            gtk::glib::timeout_add_local_once(
                std::time::Duration::from_millis(2500),
                move || {
                    reader.pad(kaeri_model::input::ReaderPad::Settings);
                },
            );
            4.5
        }
        // The page between chapters: on to chapter 14, and after the last chapter.
        "read-end" | "read-end-none" => {
            read(shell, MANGA, if shot == "read-end" { "13" } else { "15" });
            later(shell, 2500, |reader| reader.shot_end());
            4.0
        }
        // The end page with no first page to show: the plate with the chapter's number.
        "read-end-plate" => {
            read(shell, MANGA, "13");
            later(shell, 2500, |reader| reader.shot_end_plate());
            4.0
        }
        // The next chapter an extra, 16.5, with Skip to chapter 17 beside Continue.
        "read-end-skip" => {
            read(shell, MANGA, "13");
            later(shell, 2500, |reader| reader.shot_end());
            // After the engine's own end page has come, which names no skip.
            later(shell, 3500, |reader| reader.shot_end_skip());
            5.0
        }
        // A page still loading after 4 s: its plate, number and line.
        "read-plate" => {
            read(shell, MANGA, "13");
            later(shell, 2500, |reader| reader.shot_plate());
            4.0
        }
        // A chapter that could not open: a series no source has.
        "read-open-again" => {
            read(shell, MANGA, "999");
            later(shell, 3000, |reader| reader.shot_try_again());
            5.0
        }
        "read-open-failed" => {
            read(shell, "a:900399", "1");
            4.0
        }
        // Aa with Reading mode's list pushed, on a manga and on a strip.
        "read-mode" | "read-strip-mode" => {
            if shot == "read-mode" {
                read(shell, MANGA, "13");
            } else {
                read(shell, STRIP, "2");
            }
            later(shell, 2500, |reader| reader.shot_mode_list());
            4.5
        }
        // A page turn 60 ms and 140 ms in: the page under the one that fades away.
        "read-turn-a" | "read-turn-b" => {
            read(shell, MANGA, "13");
            later(shell, 3000, |reader| reader.pad(ReaderPad::Turn(Direction::Down)));
            if shot == "read-turn-a" { 3.0 + 0.06 } else { 3.0 + 0.14 }
        }
        "read-strip" => {
            read(shell, STRIP, "2");
            4.0
        }
        // Strip images still loading: each plate with its number, and from 4 s the line under it.
        "read-strip-plate" | "read-strip-plate-slow" => {
            read(shell, STRIP, "2");
            let line = shot == "read-strip-plate-slow";
            later(shell, 2500, move |reader| reader.shot_strip_plate(line));
            // Before the 4 s line of an image asked for at open fades in over the staged one.
            3.5
        }
        // After the last chapter there is, the strip ends on the end page.
        "read-strip-end" => {
            read(shell, STRIP, "3");
            later(shell, 2500, |reader| reader.shot_end());
            4.0
        }
        // The marker into chapter 3 coming up, and chapter 3 run on under it once the marker
        // crossed the middle, its first images in.
        "read-strip-marker" => {
            read(shell, STRIP, "2");
            later(shell, 2500, |reader| reader.shot_marker(0.6));
            4.0
        }
        "read-strip-flowed" => {
            read(shell, STRIP, "2");
            later(shell, 2500, |reader| reader.shot_marker(0.15));
            6.0
        }
        // Space in a strip 60 ms and 140 ms in: the slide on the quick spring.
        "read-strip-slide-a" | "read-strip-slide-b" => {
            read(shell, STRIP, "2");
            later(shell, 3000, |reader| reader.pad(ReaderPad::Turn(Direction::Down)));
            if shot == "read-strip-slide-a" { 3.0 + 0.06 } else { 3.0 + 0.14 }
        }
        "read-strip-light" => {
            shell.reader.shot_setting(ReaderSettingKey::Background, "white");
            read(shell, STRIP, "2");
            4.0
        }
        // Chapters over a manga page: the page grid, then the chapters by volume. The long one
        // is the reading-long scene's chapter 60 of 120.
        "read-chapters" | "read-chapters-list" | "read-chapters-long" => {
            read(shell, MANGA, if shot == "read-chapters-long" { "60" } else { "13" });
            let reader = Rc::clone(&shell.reader);
            let tab = if shot == "read-chapters" { "pages" } else { "chapters" };
            gtk::glib::timeout_add_local_once(
                std::time::Duration::from_millis(2500),
                move || reader.shot_chapters(tab),
            );
            5.0
        }
        // The novel in each look the board draws, with the chrome up: the theme's (dark), Light,
        // and Paper with Aa open.
        "read-novel-notice" | "read-novel-notice-hidden" => {
            read(shell, NOVEL, "3");
            let chrome = shot == "read-novel-notice";
            later(shell, 3000, move |reader| reader.shot_notice(chrome));
            4.5
        }
        "read-novel-end" => {
            read(shell, NOVEL, "3");
            later(shell, 3000, |reader| reader.shot_novel_end());
            30.0
        }
        "read-novel" => {
            shell.reader.shot_setting(ReaderSettingKey::Look, "system");
            read(shell, NOVEL, "3");
            shell.reader.shot_chrome(true);
            7.0
        }
        "read-novel-light" => {
            shell.reader.shot_setting(ReaderSettingKey::Look, "light");
            read(shell, NOVEL, "3");
            shell.reader.shot_chrome(true);
            7.0
        }
        "read-novel-paper" => {
            shell.reader.shot_setting(ReaderSettingKey::Look, "paper");
            read(shell, NOVEL, "3");
            shell.reader.shot_chrome(true);
            let reader = Rc::clone(&shell.reader);
            gtk::glib::timeout_add_local_once(
                std::time::Duration::from_millis(5000),
                move || {
                    reader.pad(kaeri_model::input::ReaderPad::Settings);
                },
            );
            7.5
        }
        "read-send" => {
            series(shell, MANGA);
            crate::reading::export::open(&shell.ctx, &WorkId(MANGA.to_owned()));
            3.0
        }
        _ => return None,
    })
}

/// Leave what `shot` opened.
pub fn unstage(shell: &Rc<Shell>, shot: &str) {
    if !NAMES.contains(&shot) {
        return;
    }
    if let Some(dialog) = shell.ctx.window.visible_dialog() {
        dialog.force_close();
    }
    shell.reader.shot_chrome(false);
    shell.reader.close();
    // Settings go back to the defaults the next shot expects.
    shell.reader.shot_setting(ReaderSettingKey::Background, "theme");
    shell.reader.shot_setting(ReaderSettingKey::CropBorders, "off");
    shell.reader.shot_setting(ReaderSettingKey::PageLayout, "auto");
    shell.reader.shot_setting(ReaderSettingKey::Fit, "screen");
    shell.reader.shot_setting(ReaderSettingKey::Look, "system");
    shell.go(Dest::Read);
}
